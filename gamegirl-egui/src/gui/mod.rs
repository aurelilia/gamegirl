// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod debugger_gga;
mod debugger_ggc;
mod file_dialog;
mod input;
mod options;

#[cfg(feature = "savestates")]
mod rewind;

#[cfg(feature = "remote-debugger")]
use std::sync::RwLock;
use std::{
    fs, mem,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

use cpal::Stream;
use eframe::{
    egui::{
        self, load::SizedTexture, util::History, vec2, widgets, Context, Event, ImageData, Layout,
        TextureOptions, Ui,
    },
    emath::Align,
    epaint::{ColorImage, ImageDelta, TextureId},
    CreationContext, Frame, Storage, Theme,
};
#[cfg(feature = "remote-debugger")]
use gamegirl::remote_debugger::DebuggerStatus;
use gamegirl::{gga::GameGirlAdv, ggc::GameGirl, System};

use crate::{
    gui::{
        debugger_ggc::VisualDebugState, file_dialog::File, input::InputAction, options::Options,
    },
    Colour,
};

/// Total count of windows in GUI.
const WINDOW_COUNT: usize = DBG_WINDOW_COUNT + APP_WINDOW_COUNT;

/// Function signature for a debug window
type DbgFn<G> = fn(&mut G, &mut Ui);
/// Count of debugger GUI windows that take a system as a parameter.
const DBG_WINDOW_COUNT: usize = 4;
/// Debugger GUI windows. Both GGC and GGA versions for each.
const DBG_WINDOWS: [(&str, DbgFn<GameGirl>, DbgFn<GameGirlAdv>); DBG_WINDOW_COUNT] = [
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

/// Function signature for an app window
type AppFn = fn(&mut App, &Context, &mut Ui);
/// Count of GUI windows that take the App as a parameter.
const APP_WINDOW_COUNT: usize = 5;
/// GUI windows that take the App as a parameter.
const APP_WINDOWS: [(&str, AppFn); APP_WINDOW_COUNT] = [
    ("Options", options::options),
    ("About", options::about),
    ("VRAM", debugger_ggc::vram_viewer),
    ("Background Map", debugger_ggc::bg_map_viewer),
    ("Remote Debugger", debugger_gga::remote_debugger),
];

/// Start the GUI.
#[cfg(not(target_arch = "wasm32"))]
pub fn start() {
    let options = eframe::NativeOptions {
        transparent: true,
        default_theme: Theme::Dark,
        ..Default::default()
    };
    eframe::run_native("gamegirl", options, Box::new(|ctx| make_app(ctx))).unwrap()
}

/// Start the GUI.
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), eframe::wasm_bindgen::JsValue> {
    let options = eframe::WebOptions {
        default_theme: Theme::Dark,
        ..Default::default()
    };
    eframe::WebRunner::new()
        .start("the_canvas_id", options, Box::new(|ctx| make_app(ctx)))
        .await
}

fn make_app(ctx: &CreationContext<'_>) -> Box<App> {
    let state: State = {
        #[cfg(feature = "persistence")]
        {
            ctx.storage
                .and_then(|s| eframe::get_value(s, "gamegirl_data"))
                .unwrap_or_default()
        }

        #[cfg(not(feature = "persistence"))]
        State::default()
    };

    let gg = System::default();
    let gg = Arc::new(Mutex::new(gg));

    let texture = App::make_screen_texture(&ctx.egui_ctx, [160, 144], state.options.tex_filter);
    let mut app = App {
        gg,
        current_rom_path: None,
        #[cfg(feature = "savestates")]
        rewinder: rewind::Rewinding::default(),
        visual_debug: VisualDebugState::default(),
        #[cfg(feature = "remote-debugger")]
        remote_dbg: Arc::new(RwLock::new(DebuggerStatus::NotActive)),
        fast_forward_toggled: false,

        texture,
        window_states: [false; WINDOW_COUNT],
        message_channel: mpsc::channel(),
        frame_times: History::new(0..120, 2.0),
        last_time: 0.0,
        audio_stream: None,

        state,
    };

    #[cfg(feature = "savestates")]
    app.setup_rewind();

    Box::new(app)
}

/// The App state.
struct App {
    /// The GG currently running.
    gg: Arc<Mutex<System>>,
    /// The path to the ROM currently running, if any. Always None on WASM.
    current_rom_path: Option<PathBuf>,
    /// Rewinder state.
    #[cfg(feature = "savestates")]
    rewinder: rewind::Rewinding,
    /// State for visual debugging tools.
    visual_debug: VisualDebugState,
    /// Remote debugger status.
    #[cfg(feature = "remote-debugger")]
    remote_dbg: Arc<RwLock<DebuggerStatus>>,
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
    /// Last frame time according to egui's input subsystem.
    last_time: f32,
    /// Stream for audio.
    audio_stream: Option<Stream>,

    /// The App state, which is persisted on reboot.
    state: State,
}

impl App {
    #[cfg(feature = "savestates")]
    fn setup_rewind(&mut self) {
        self.rewinder
            .set_rw_buf_size(self.state.options.rewind_buffer_size);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        let size = self.update_gg(ctx);
        self.process_messages();

        egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                let now = { ctx.input(|i| i.time) };
                self.navbar(now, frame, ui);
            });
        });

        egui::Window::new("GameGirl")
            .resizable(false)
            .show(ctx, |ui| {
                ui.image(Into::<SizedTexture>::into((
                    self.texture,
                    vec2(
                        (size[0] * self.state.options.display_scale) as f32,
                        (size[1] * self.state.options.display_scale) as f32,
                    ),
                )));
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
                _ => todo!(),
            };
        }

        // Immediately repaint, since the GG will have a new frame.
        // egui will automatically bind the framerate to VSYNC.
        ctx.request_repaint();
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        self.save_game();

        #[cfg(feature = "persistence")]
        {
            eframe::set_value(_storage, "gamegirl_data", &self.state);
        }
    }
}

impl App {
    /// Update the system's state.
    /// Returns screen dimensions.
    fn update_gg(&mut self, ctx: &Context) -> [usize; 2] {
        let (frame, size) = self.get_gg_frame(ctx);
        if let Some(pixels) = frame {
            let img = ImageDelta::full(
                ImageData::Color(ColorImage { size, pixels }.into()), // todo meh
                self.state.options.tex_filter,
            );
            let manager = ctx.tex_manager();
            manager.write().set(self.texture, img);
        }
        size
    }

    /// Process keyboard inputs and return the GG's next frame, if one was
    /// produced.
    fn get_gg_frame(&mut self, ctx: &Context) -> (Option<Vec<Colour>>, [usize; 2]) {
        let time = ctx.input(|i| {
            for event in &i.events {
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
            i.time as f32
        });
        // slightly lower than actual delta - we want to keep pace with audio,
        // so make sure we don't run ahead of it.
        let delta = (time - self.last_time).min(0.1) - 0.0001;
        self.last_time = time;

        let mut gg = self.gg.lock().unwrap();
        let size = gg.screen_size();

        #[cfg(feature = "savestates")]
        {
            if self.rewinder.rewinding {
                let frame = if let Some(state) = self.rewinder.rewind_buffer.pop() {
                    gg.load_state(state);
                    gg.options().invert_audio_samples = true;
                    gg.produce_frame()
                } else {
                    self.rewinder.rewinding = false;
                    gg.options().invert_audio_samples = false;
                    gg.last_frame()
                };
                (frame.map(|p| unsafe { mem::transmute(p) }), size)
            } else {
                gg.advance_delta(delta);
                let frame = gg.last_frame().map(|p| unsafe { mem::transmute(p) });
                if frame.is_some() {
                    let state = gg.save_state();
                    self.rewinder.rewind_buffer.push(state);
                }
                (frame, size)
            }
        }
        #[cfg(not(feature = "savestates"))]
        {
            gg.advance_delta(delta);
            (gg.last_frame().map(|p| unsafe { mem::transmute(p) }), size)
        }
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self) {
        while let Ok(Message::FileOpen(file)) = self.message_channel.1.try_recv() {
            self.save_game();
            self.gg.lock().unwrap().load_cart(
                file.content,
                file.path.clone(),
                &self.state.options.gg,
            );

            if self.audio_stream.is_none() {
                self.audio_stream = crate::setup_cpal(self.gg.clone());
            }

            self.current_rom_path = file.path.clone();
            if let Some(path) = file.path {
                if let Some(existing) = self.state.last_opened.iter().position(|p| *p == path) {
                    self.state.last_opened.swap(0, existing);
                } else {
                    self.state.last_opened.insert(0, path);
                    self.state.last_opened.truncate(10);
                }
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
    fn navbar(&mut self, now: f64, frame: &mut Frame, ui: &mut Ui) {
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
                    frame.close();
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

            #[cfg(feature = "remote-debugger")]
            {
                ui.separator();
                self.window_states[8] |= ui.button("Remote Debugger").clicked();
            }
        });

        #[cfg(feature = "savestates")]
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

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
    fn make_screen_texture(ctx: &Context, size: [usize; 2], filter: TextureOptions) -> TextureId {
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
#[derive(Default)]
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
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
