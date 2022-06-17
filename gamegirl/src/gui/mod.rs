mod debugger_gga;
mod debugger_ggc;
mod file_dialog;
mod input;
mod options;
mod rewind;

use crate::{
    gui::{
        debugger_ggc::VisualDebugState, file_dialog::File, input::InputAction, options::Options,
        rewind::Rewinding,
    },
    Colour,
};
use core::{
    common::{BorrowedSystem, System},
    gga::GameGirlAdv,
    ggc::GameGirl,
};
use eframe::{
    egui::{
        self, util::History, vec2, widgets, Context, Event, ImageData, Layout, TextureFilter, Ui,
        Vec2,
    },
    epaint::{ColorImage, ImageDelta, TextureId},
    epi,
    epi::{Frame, Storage},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, mem,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

/// How long a frame takes, and how much the GG should be advanced
/// each frame. TODO: This assumption only holds for 60hz devices!
const FRAME_LEN: Duration = Duration::from_secs_f64(1.0 / 60.0);

/// Total count of windows in GUI.
const WINDOW_COUNT: usize = DBG_WINDOW_COUNT + APP_WINDOW_COUNT;

/// Count of debugger GUI windows that take a system as a parameter.
const DBG_WINDOW_COUNT: usize = 4;
/// Debugger GUI windows. Both GGC and GGA versions for each.
const DBG_WINDOWS: [(
    &str,
    fn(&mut GameGirl, &mut Ui),
    fn(&mut GameGirlAdv, &mut Ui),
); DBG_WINDOW_COUNT] = [
    ("Debugger", debugger_ggc::debugger, debugger_gga::debugger),
    (
        "Breakpoints",
        debugger_ggc::breakpoints,
        debugger_gga::breakpoints,
    ),
    ("Memory", debugger_ggc::memory, debugger_gga::memory),
    (
        "Cartridge",
        debugger_ggc::cart_info,
        debugger_gga::cart_info,
    ),
];

/// Count of GUI windows that take the App as a parameter.
const APP_WINDOW_COUNT: usize = 4;
/// GUI windows that take the App as a parameter.
const APP_WINDOWS: [(&str, fn(&mut App, &Context, &mut Ui)); APP_WINDOW_COUNT] = [
    ("Options", options::options),
    ("About", options::about),
    ("VRAM", debugger_ggc::vram_viewer),
    ("Background Map", debugger_ggc::bg_map_viewer),
];

/// Start the GUI. Since this is native, this call will never return.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(gg: Arc<Mutex<System>>) {
    let options = eframe::NativeOptions {
        transparent: true,
        ..Default::default()
    };
    eframe::run_native(Box::new(make_app(gg)), options)
}

/// Start the GUI. Since this is WASM, this call will return.
#[cfg(target_arch = "wasm32")]
pub fn start(gg: Arc<Mutex<System>>, canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    eframe::start_web(canvas_id, Box::new(make_app(gg)))
}

fn make_app(gg: Arc<Mutex<System>>) -> App {
    App {
        gg,
        current_rom_path: None,
        rewinder: Rewinding::default(),
        visual_debug: VisualDebugState::default(),
        fast_forward_toggled: false,

        texture: TextureId::default(),
        window_states: [false; WINDOW_COUNT],
        message_channel: mpsc::channel(),
        frame_times: History::new(0..120, 2.0),

        state: State {
            last_opened: vec![],
            options: Options::default(),
        },
    }
}

/// The App state.
struct App {
    /// The GG currently running.
    gg: Arc<Mutex<System>>,
    /// The path to the ROM currently running, if any. Always None on WASM.
    current_rom_path: Option<PathBuf>,
    /// Rewinder state.
    rewinder: Rewinding,
    /// State for visual debugging tools.
    visual_debug: VisualDebugState,
    /// If the emulator is fast-forwarding using the toggle hotkey.
    fast_forward_toggled: bool,

    /// Texture for the GG's PPU output.
    texture: TextureId,
    /// Open/closed states of all windows.
    window_states: [bool; WINDOW_COUNT],
    /// Message channel for reacting to some async events, see [Message].
    message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
    /// Frame times.
    frame_times: History<f32>,

    /// The App state, which is persisted on reboot.
    state: State,
}

impl epi::App for App {
    fn update(&mut self, ctx: &Context, frame: &Frame) {
        let size = self.update_gg(ctx, FRAME_LEN);
        self.process_messages();

        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                let now = { ctx.input().time };
                self.navbar(now, frame, ui);
            });
        });

        egui::Window::new("GameGirl")
            .resizable(false)
            .show(ctx, |ui| {
                ui.image(
                    self.texture,
                    [
                        (size[0] * self.state.options.display_scale) as f32,
                        (size[1] * self.state.options.display_scale) as f32,
                    ],
                );
            });

        let mut states = self.window_states;
        for ((name, runner), state) in APP_WINDOWS
            .iter()
            .zip(states.iter_mut().skip(DBG_WINDOW_COUNT))
        {
            egui::Window::new(*name)
                .open(state)
                .show(ctx, |ui| runner(self, ctx, ui));
        }
        self.window_states = states;

        let mut gg = self.gg.lock().unwrap();
        for ((name, ggc, gga), state) in DBG_WINDOWS.iter().zip(self.window_states.iter_mut()) {
            let win = egui::Window::new(*name).open(state);
            match &mut *gg {
                System::GGC(gg) => win.show(ctx, |ui| ggc(gg, ui)),
                System::GGA(gg) => win.show(ctx, |ui| gga(gg, ui)),
            };
        }

        // Immediately repaint, since the GG will have a new frame.
        // egui will automatically bind the framerate to VSYNC.
        ctx.request_repaint();
    }

    fn setup(&mut self, ctx: &Context, _frame: &Frame, storage: Option<&dyn Storage>) {
        if let Some(state) = storage.and_then(|s| epi::get_value(s, "gamegirl_data")) {
            self.state = state;
        }

        self.texture = Self::make_screen_texture(ctx, [160, 144], self.state.options.tex_filter);

        self.rewinder
            .set_rw_buf_size(self.state.options.rewind_buffer_size);
        let buffer = self.rewinder.rewind_buffer.clone();
        if self.state.options.enable_rewind {
            self.gg.lock().unwrap().options().frame_finished = Box::new(move |gg| {
                // Kinda ugly duplication but it works ig?
                match gg {
                    BorrowedSystem::GGC(gg) if !gg.options.invert_audio_samples => {
                        buffer.lock().unwrap().push(gg.save_state())
                    }
                    BorrowedSystem::GGA(gg) if !gg.options.invert_audio_samples => {
                        buffer.lock().unwrap().push(gg.save_state())
                    }
                    _ => (),
                }
            });
        }
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        self.save_game();
        epi::set_value(storage, "gamegirl_data", &self.state);
    }

    fn name(&self) -> &str {
        "GameGirl"
    }

    fn max_size_points(&self) -> Vec2 {
        vec2(4000.0, 4000.0)
    }
}

impl App {
    /// Update the system's state.
    /// Returns screen dimensions.
    fn update_gg(&mut self, ctx: &Context, advance_by: Duration) -> [usize; 2] {
        let (frame, size) = self.get_gg_frame(ctx, advance_by);
        if let Some(pixels) = frame {
            let img = ImageDelta::full(ImageData::Color(ColorImage { size, pixels }));
            let manager = ctx.tex_manager();
            manager.write().set(self.texture, img);
        }
        size
    }

    /// Process keyboard inputs and return the GG's next frame, if one was
    /// produced.
    fn get_gg_frame(
        &mut self,
        ctx: &Context,
        advance_by: Duration,
    ) -> (Option<Vec<Colour>>, [usize; 2]) {
        for event in &ctx.input().events {
            if let Event::Key { key, pressed, .. } = event {
                if let Some(action) = self.state.options.input.pending.take() {
                    self.state.options.input.set_key(*key, action);
                    continue;
                }

                match self.state.options.input.get_key(*key) {
                    Some(InputAction::Button(btn)) => {
                        self.gg.lock().unwrap().set_button(btn, *pressed)
                    }
                    Some(InputAction::Hotkey(idx)) => {
                        input::HOTKEYS[idx as usize].1(self, *pressed)
                    }
                    None => (),
                }
            }
        }

        let mut gg = self.gg.lock().unwrap();
        let size = gg.screen_size();
        if self.rewinder.rewinding {
            if let Some(state) = self.rewinder.rewind_buffer.lock().unwrap().pop() {
                gg.load_state(state);
                gg.options().invert_audio_samples = true;
                return (
                    gg.produce_frame().map(|p| unsafe { mem::transmute(p) }),
                    size,
                );
            } else {
                self.rewinder.rewinding = false;
                gg.options().invert_audio_samples = false;
            }
        } else {
            gg.advance_delta(advance_by.as_secs_f32());
        }
        (gg.last_frame().map(|p| unsafe { mem::transmute(p) }), size)
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self) {
        loop {
            match self.message_channel.1.try_recv() {
                Ok(Message::FileOpen(file)) => {
                    self.save_game();
                    self.gg.lock().unwrap().load_cart(
                        file.content,
                        file.path.clone(),
                        &self.state.options.gg,
                    );

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
        self.gg
            .lock()
            .unwrap()
            .save_game(self.current_rom_path.clone());
    }

    /// Paint the navbar.
    fn navbar(&mut self, now: f64, frame: &Frame, ui: &mut Ui) {
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
                gg.options().running = !gg.options().running && gg.options().rom_loaded;
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
                    frame.quit();
                }
            }
        });

        ui.menu_button("Debugger", |ui| {
            self.window_states[0] |= ui.button("Debugger").clicked();
            self.window_states[1] |= ui.button("Breakpoints").clicked();
            self.window_states[2] |= ui.button("Memory Viewer").clicked();
            self.window_states[3] |= ui.button("Cartridge Viewer").clicked();
            ui.separator();
            self.window_states[6] |= ui.button("VRAM Viewer").clicked();
            self.window_states[7] |= ui.button("Background Map Viewer").clicked();
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

        ui.with_layout(Layout::right_to_left(), |ui| {
            let time = frame.info().cpu_usage.unwrap_or(0.0);
            self.frame_times.add(now, time);
            // Backwards because we're in RTL layout
            ui.monospace(format!(
                "{:.3}ms",
                self.frame_times.average().unwrap_or(0.0) * 1000.0
            ));
            ui.label("Frame time: ");
        });
    }

    /// Create the screen texture.
    fn make_screen_texture(ctx: &Context, size: [usize; 2], filter: TextureFilter) -> TextureId {
        let manager = ctx.tex_manager();
        let id = manager.write().alloc(
            "screen".into(),
            ColorImage::new(size, Colour::BLACK).into(),
            filter,
        );
        id
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
    /// A file picked by the user to be opend as a ROM, from the "Open ROM" file
    /// picker dialog.
    FileOpen(File),
}
