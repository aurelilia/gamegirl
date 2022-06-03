mod debugger;
mod file_dialog;
mod options;
mod rewind;

use crate::gui::file_dialog::File;
use crate::gui::options::Options;
use crate::gui::rewind::Rewinding;
use crate::storage::Storage as CartStore;
use crate::system::io::cartridge::Cartridge;
use crate::system::io::joypad::{Button, Joypad};
use crate::Colour;
use crate::GameGirl;
use eframe::egui::{self, widgets, Context, Event, ImageData, Key, Ui};
use eframe::egui::{vec2, TextureFilter, Vec2};
use eframe::epaint::{ColorImage, ImageDelta, TextureId};
use eframe::epi;
use eframe::epi::{Frame, Storage};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// How long a frame takes, and how much the GG should be advanced
/// each frame. TODO: This assumption only holds for 60hz devices!
const FRAME_LEN: Duration = Duration::from_secs_f64(1.0 / 60.0);

/// Total count of windows in GUI.
const WINDOW_COUNT: usize = GG_WINDOW_COUNT + STATE_WINDOW_COUNT;

/// Count of GUI windows that take the GG as a parameter.
/// For now, this is only the debugger's windows.
const GG_WINDOW_COUNT: usize = 4;
/// GUI windows that take the GG as parameter.
const GG_WINDOWS: [(&str, fn(&mut GameGirl, &mut Ui)); GG_WINDOW_COUNT] = [
    ("Debugger", debugger::debugger),
    ("Breakpoints", debugger::breakpoints),
    ("Memory", debugger::memory),
    ("Cartridge", debugger::cart_info),
];

/// Count of GUI windows that take the App state as a parameter.
const STATE_WINDOW_COUNT: usize = 2;
/// GUI windows that take the App state as a parameter.
const STATE_WINDOWS: [(&str, fn(&Context, &mut State, &mut Ui)); STATE_WINDOW_COUNT] =
    [("Options", options::options), ("About", options::about)];

/// Start the GUI. Since this is native, this call will never return.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(gg: Arc<Mutex<GameGirl>>) {
    let options = eframe::NativeOptions {
        transparent: true,
        ..Default::default()
    };
    eframe::run_native(Box::new(make_app(gg)), options)
}

/// Start the GUI. Since this is WASM, this call will return.
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
        current_rom_path: None,
        rewinder: Rewinding::default(),

        texture: TextureId::default(),
        window_states: [false; WINDOW_COUNT],
        message_channel: mpsc::channel(),

        state: State {
            last_opened: vec![],
            options: Options::default(),
        },
    }
}

/// The App state.
struct App {
    /// The GG currently running.
    gg: Arc<Mutex<GameGirl>>,
    /// The path to the ROM currently running, if any. Always None on WASM.
    current_rom_path: Option<PathBuf>,
    /// Rewinder state.
    rewinder: Rewinding,

    /// Texture for the GG's PPU output.
    texture: TextureId,
    /// Open/closed states of all windows.
    window_states: [bool; WINDOW_COUNT],
    /// Message channel for reacting to some async events, see [Message].
    message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),

    /// The App state, which is persisted on reboot.
    state: State,
}

impl epi::App for App {
    fn update(&mut self, ctx: &Context, frame: &Frame) {
        self.update_gg(ctx, FRAME_LEN);
        self.process_messages();

        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                self.navbar(frame, ui);
            });
        });

        egui::Window::new("GameGirl")
            .resizable(false)
            .show(ctx, |ui| {
                ui.image(self.texture, [320.0, 288.0]);
            });

        for ((name, runner), state) in STATE_WINDOWS
            .iter()
            .zip(self.window_states.iter_mut().skip(GG_WINDOW_COUNT))
        {
            egui::Window::new(*name)
                .open(state)
                .show(ctx, |ui| runner(ctx, &mut self.state, ui));
        }

        let mut gg = self.gg.lock().unwrap();
        for ((name, runner), state) in GG_WINDOWS.iter().zip(self.window_states.iter_mut()) {
            egui::Window::new(*name)
                .open(state)
                .show(ctx, |ui| runner(&mut gg, ui));
        }

        // Immediately repaint, since the GG will have a new frame.
        // egui will automatically bind the framerate to VSYNC.
        ctx.request_repaint();
    }

    fn setup(&mut self, ctx: &Context, _frame: &Frame, storage: Option<&dyn Storage>) {
        let manager = ctx.tex_manager();
        self.texture = manager.write().alloc(
            "screen".into(),
            ColorImage::new([160, 144], Colour::BLACK).into(),
            TextureFilter::Nearest,
        );
        if let Some(state) = storage.and_then(|s| epi::get_value(s, "gamelin_data")) {
            self.state = state;
        }
        self.rewinder.set_rw_buf_size(self.state.options.rewind_buffer_size);
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        self.save_game();
        epi::set_value(storage, "gamelin_data", &self.state);
    }

    fn name(&self) -> &str {
        "GameGirl"
    }

    fn max_size_points(&self) -> Vec2 {
        vec2(4000.0, 4000.0)
    }
}

impl App {
    /// Update the system's state
    fn update_gg(&mut self, ctx: &Context, advance_by: Duration) {
        let frame = self.get_gg_frame(ctx, advance_by);
        if let Some(data) = frame {
            let img = ImageDelta::full(ImageData::Color(ColorImage {
                size: [160, 144],
                pixels: data,
            }));
            let manager = ctx.tex_manager();
            manager.write().set(self.texture, img);
        }
    }

    /// Process keyboard inputs and return the GG's next frame, if one was produced.
    fn get_gg_frame(&mut self, ctx: &Context, advance_by: Duration) -> Option<Vec<Colour>> {
        let mut gg = self.gg.lock().unwrap();
        for event in &ctx.input().events {
            if let Event::Key { key, pressed, .. } = event {
                if let Some(button) = Button::from_key(*key) {
                    Joypad::set(&mut gg, button, *pressed);
                }
                if *key == Key::R {
                    self.rewinder.rewinding = *pressed;
                    gg.invert_audio_samples = *pressed;
                }
            }
        }

        if self.rewinder.rewinding {
            if let Some(state) = self.rewinder.rewind_buffer.pop() {
                gg.load_state(state);
                gg.invert_audio_samples = true;
                // Produce a frame
                gg.advance_delta(advance_by.as_secs_f32());
            } else {
                self.rewinder.rewinding = false;
                gg.invert_audio_samples = false;
            }
        } else {
            gg.advance_delta(advance_by.as_secs_f32());
            if self.state.options.enable_rewind {
                self.rewinder.rewind_buffer.push(gg.save_state());
            }
        }
        gg.mmu.ppu.last_frame.take()
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self) {
        loop {
            match self.message_channel.1.try_recv() {
                Ok(Message::FileOpen(file)) => {
                    self.save_game();
                    let mut cart = Cartridge::from_rom(file.content);
                    CartStore::load(file.path.clone(), &mut cart);
                    self.gg
                        .lock()
                        .unwrap()
                        .load_cart(cart, &self.state.options.gg, true);

                    self.current_rom_path = file.path.clone();
                    if let Some(path) = file.path {
                        if let Some(existing) =
                            self.state.last_opened.iter().position(|p| *p == path)
                        {
                            self.state.last_opened.swap(0, existing);
                        } else {
                            self.state.last_opened.insert(0, path);
                            self.state.last_opened.truncate(10);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Save the system cart RAM, if a cart is loaded and it has RAM.
    fn save_game(&self) {
        let gg = self.gg.lock().unwrap();
        if gg.mmu.cart.rom.len() > 0 && gg.mmu.cart.ram_bank_count() > 0 {
            CartStore::save(self.current_rom_path.clone(), &gg.mmu.cart);
        }
    }

    /// Paint the navbar.
    fn navbar(&mut self, _frame: &Frame, ui: &mut Ui) {
        widgets::global_dark_light_mode_switch(ui);
        ui.separator();

        ui.menu_button("File", |ui| {
            if ui.button("Open ROM").clicked() {
                file_dialog::open(self.message_channel.0.clone());
                ui.close_menu();
            }
            if !self.state.last_opened.is_empty() {
                ui.menu_button("Last Opened", |ui| {
                    for path in &self.state.last_opened {
                        if ui
                            .button(path.file_name().unwrap().to_str().unwrap())
                            .clicked()
                        {
                            self.message_channel
                                .0
                                .send(Message::FileOpen(File {
                                    content: fs::read(path).unwrap(),
                                    path: Some(path.clone()),
                                }))
                                .ok();
                            ui.close_menu();
                        }
                    }
                });
            }
            ui.separator();

            if ui.button("Save").clicked() {
                self.save_game();
                ui.close_menu();
            }
            if ui.button("Pause").clicked() {
                let mut gg = self.gg.lock().unwrap();
                gg.running = !gg.running && gg.rom_loaded;
                ui.close_menu();
            }
            if ui.button("Reset").clicked() {
                self.gg.lock().unwrap().reset();
                ui.close_menu();
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.separator();
                if ui.button("Exit").clicked() {
                    _frame.quit();
                }
            }
        });

        ui.menu_button("Debugger", |ui| {
            if ui.button("Debugger").clicked() {
                self.window_states[0] = true;
            }
            if ui.button("Breakpoints").clicked() {
                self.window_states[1] = true;
            }
            if ui.button("Memory Viewer").clicked() {
                self.window_states[2] = true;
            }
            if ui.button("Cartridge Viewer").clicked() {
                self.window_states[3] = true;
            }
        });

        ui.menu_button("Savestates", |ui| {
            for (i, state) in self.rewinder.save_states.iter_mut().enumerate() {
                if ui.button(format!("Save State {}", i + 1)).clicked() {
                    *state = Some(self.gg.lock().unwrap().save_state());
                    ui.close_menu();
                }
            }
            ui.separator();

            for (i, state) in self
                .rewinder
                .save_states
                .iter()
                .filter_map(|s| s.as_ref())
                .enumerate()
            {
                if ui.button(format!("Load State {}", i + 1)).clicked() {
                    let mut gg = self.gg.lock().unwrap();
                    self.rewinder.before_last_ss_load = Some(gg.save_state());
                    gg.load_state(state);
                    ui.close_menu();
                }
            }
        });

        ui.menu_button("Options", |ui| {
            if ui.button("Options").clicked() {
                self.window_states[4] = true;
                ui.close_menu();
            }
            if ui.button("About").clicked() {
                self.window_states[5] = true;
                ui.close_menu();
            }
        });
    }
}

/// State that is persisted on app reboot.
#[derive(Serialize, Deserialize)]
pub struct State {
    /// A list of last opened ROMs. Size is capped to 10, last opened
    /// ROM is at index 0. The oldest ROM gets removed first.
    last_opened: Vec<PathBuf>,
    /// User configuration options.
    options: Options,
}

/// A message that can be sent from some async context.
pub enum Message {
    /// A file picked by the user to be opend as a ROM, from the "Open ROM" file picker dialog.
    FileOpen(File),
}
