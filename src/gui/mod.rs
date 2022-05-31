mod debugger;
mod file_dialog;

use crate::system::io::joypad::{Button, Joypad};
use crate::Colour;
use crate::GameGirl;
use eframe::egui::TextureFilter;
use eframe::egui::{self, widgets, Context, Event, ImageData, Ui};
use eframe::epaint::{ColorImage, ImageDelta, TextureId};
use eframe::epi;
use eframe::epi::{Frame, Storage};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const FRAME_LEN: Duration = Duration::from_secs_f64(1.0 / 60.0);

const WINDOW_COUNT: usize = 1;
const WINDOWS: [(&str, fn(&GameGirl, &mut Ui)); WINDOW_COUNT] =
    [("Registers", debugger::registers)];

#[cfg(not(target_arch = "wasm32"))]
pub fn start(gg: Arc<Mutex<GameGirl>>) {
    let options = eframe::NativeOptions {
        transparent: true,
        ..Default::default()
    };
    eframe::run_native(Box::new(make_app(gg)), options)
}

#[cfg(target_arch = "wasm32")]
pub fn start(
    gg: Arc<Mutex<GameGirl>>,
    canvas_id: &str,
) -> Result<(), eframe::wasm_bindgen::JsValue> {
    eframe::start_web(canvas_id, Box::new(make_app(gg)))
}

fn make_app(gg: Arc<Mutex<GameGirl>>) -> App {
    App {
        gg,
        texture: TextureId::default(),
        window_states: [false; WINDOW_COUNT],
        message_channel: mpsc::channel(),
    }
}

struct App {
    gg: Arc<Mutex<GameGirl>>,
    texture: TextureId,
    window_states: [bool; WINDOW_COUNT],
    message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
}

impl epi::App for App {
    fn update(&mut self, ctx: &Context, _frame: &Frame) {
        self.update_gg(ctx, FRAME_LEN);
        self.process_messages();

        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                self.navbar(ui);
            });
        });

        egui::Window::new("GameGirl")
            .resizable(false)
            .show(ctx, |ui| {
                ui.image(self.texture, [320.0, 288.0]);
            });

        let gg = self.gg.lock().unwrap();
        for ((name, runner), state) in WINDOWS.iter().zip(self.window_states.iter_mut()) {
            egui::Window::new(*name)
                .open(state)
                .show(ctx, |ui| runner(&gg, ui));
        }

        ctx.request_repaint();
    }

    fn setup(&mut self, ctx: &Context, _frame: &Frame, _storage: Option<&dyn Storage>) {
        let manager = ctx.tex_manager();
        self.texture = manager.write().alloc(
            "screen".into(),
            ColorImage::new([160, 144], Colour::BLACK).into(),
            TextureFilter::Nearest,
        );
    }

    fn name(&self) -> &str {
        "GameGirl"
    }
}

impl App {
    fn update_gg(&mut self, ctx: &Context, advance_by: Duration) {
        let frame = {
            let mut gg = self.gg.lock().unwrap();
            for event in &ctx.input().events {
                if let Event::Key { key, pressed, .. } = event {
                    if let Some(button) = Button::from_key(*key) {
                        Joypad::set(&mut gg, button, *pressed);
                    }
                }
            }

            gg.advance_delta(advance_by.as_secs_f32());
            gg.mmu.ppu.last_frame.take()
        };
        if let Some(data) = frame {
            let img = ImageDelta::full(ImageData::Color(ColorImage {
                size: [160, 144],
                pixels: data,
            }));
            let manager = ctx.tex_manager();
            manager.write().set(self.texture, img);
        }
    }

    fn process_messages(&mut self) {
        loop {
            match self.message_channel.1.try_recv() {
                Ok(Message::FileOpen(file)) => self.gg.lock().unwrap().load_cart(file, true),
                Err(_) => break,
            }
        }
    }

    fn navbar(&mut self, ui: &mut Ui) {
        widgets::global_dark_light_mode_switch(ui);
        ui.separator();

        ui.menu_button("💻 File", |ui| {
            if ui.button("Open ROM").clicked() {
                file_dialog::open(self.message_channel.0.clone());
                ui.close_menu();
            }
        });
        ui.separator();

        if ui.selectable_label(false, "Registers").clicked() {
            self.window_states[0] = true;
        }
    }
}

pub enum Message {
    FileOpen(Vec<u8>),
}
