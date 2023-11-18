// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    mem,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

use common::{misc::SystemConfig, Core};
use cpal::Stream;
use eframe::{
    egui::{Context, Event, TextureOptions},
    emath::History,
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
    glow::{self},
    CreationContext, Frame, Storage,
};

use crate::{
    gui::{self, APP_WINDOW_COUNT},
    input::{self, File, Input, InputAction},
    rewind::Rewinder,
    Colour,
};

/// The main app struct used by the GUI.
pub struct App {
    /// The core currently running.
    pub core: Arc<Mutex<Box<dyn Core>>>,
    /// The path to the ROM currently running, if any. Always None on WASM.
    pub current_rom_path: Option<PathBuf>,

    /// Rewinder state.
    pub rewinder: Rewinder,
    /// If the emulator is fast-forwarding using the toggle hotkey.
    pub fast_forward_toggled: bool,

    /// Texture(s) for the core's graphics output.
    pub textures: Vec<TextureId>,
    /// Message channel for reacting to some async events, see [Message].
    pub message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
    /// Frame times.
    pub frame_times: History<f32>,
    /// Stream for audio.
    audio_stream: Option<Stream>,
    /// App window states.
    pub app_window_states: [bool; APP_WINDOW_COUNT],
    /// Debugger window states.
    pub debugger_window_states: Vec<bool>,

    /// The App state, which is persisted on reboot.
    pub state: State,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        let size = self.update_gg(ctx);
        self.process_messages(frame.gl());
        gui::draw(self, ctx, frame, size);

        // Immediately repaint, since the GG will have a new frame.
        // egui will automatically bind the framerate to VSYNC.
        ctx.request_repaint();
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        self.save_game();
        eframe::set_value(storage, "gamegirl_data", &self.state);
    }
}

impl App {
    /// Update the system's state.
    /// Returns screen dimensions.
    fn update_gg(&mut self, ctx: &Context) -> [usize; 2] {
        let (frame, size) = self.get_frame(ctx);
        if let Some(pixels) = frame {
            let img = ImageDelta::full(
                ImageData::Color(ColorImage { size, pixels }.into()), // todo meh
                self.state.options.tex_filter,
            );
            let manager = ctx.tex_manager();
            manager.write().set(self.textures[0], img);
        }
        size
    }

    /// Process keyboard inputs and return the GG's next frame, if one was
    /// produced.
    fn get_frame(&mut self, ctx: &Context) -> (Option<Vec<Colour>>, [usize; 2]) {
        let delta = ctx.input(|i| {
            for event in &i.events {
                if let Event::Key { key, pressed, .. } = event {
                    if let Some(action) = self.state.options.input.pending.take() {
                        self.state.options.input.set_key(*key, action);
                        continue;
                    }

                    match self.state.options.input.get_key(*key) {
                        Some(InputAction::Button(btn)) => {
                            self.core.lock().unwrap().set_button(btn, *pressed)
                        }
                        Some(InputAction::Hotkey(idx)) => {
                            input::HOTKEYS[idx as usize].1(self, *pressed)
                        }
                        None => (),
                    }
                }
            }
            i.stable_dt.min(0.016).max(0.001) - 0.0009
        });

        let mut core = self.core.lock().unwrap();
        let size = core.screen_size();

        if self.rewinder.rewinding {
            let frame = if let Some(state) = self.rewinder.rewind_buffer.pop() {
                core.load_state(state);
                core.options().invert_audio_samples = true;
                core.produce_frame()
            } else {
                self.rewinder.rewinding = false;
                core.options().invert_audio_samples = false;
                core.last_frame()
            };
            (frame.map(|p| unsafe { mem::transmute(p) }), size)
        } else {
            core.advance_delta(delta);
            let frame = core.last_frame().map(|p| unsafe { mem::transmute(p) });
            if frame.is_some() {
                let state = core.save_state();
                self.rewinder.rewind_buffer.push(state);
            }
            (frame, size)
        }
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self, gl: Option<&Arc<glow::Context>>) {
        while let Ok(Message::FileOpen(file)) = self.message_channel.1.try_recv() {
            self.save_game();

            let tex = match self.textures[0] {
                TextureId::Managed(m) => m,
                _ => panic!(),
            };

            *self.core.lock().unwrap() = gamegirl::load_cart(
                file.content,
                file.path.clone(),
                &self.state.options.sys,
                gl.cloned(),
                tex as u32,
            );

            self.audio_stream = crate::setup_cpal(self.core.clone());

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
    pub fn save_game(&self) {
        gamegirl::save_game(&**self.core.lock().unwrap(), self.current_rom_path.clone());
    }

    pub fn new(ctx: &CreationContext<'_>) -> Box<Self> {
        let state: State = ctx
            .storage
            .and_then(|s| eframe::get_value(s, "gamegirl_data"))
            .unwrap_or_default();
        let core = gamegirl::dummy_core();
        let core = Arc::new(Mutex::new(core));
        let textures = vec![App::make_screen_texture(
            &ctx.egui_ctx,
            [160, 144],
            state.options.tex_filter,
        )];
        Box::new(App {
            core,
            current_rom_path: None,

            rewinder: Rewinder::new(state.options.rewind_buffer_size),
            fast_forward_toggled: false,
            app_window_states: [false; APP_WINDOW_COUNT],
            debugger_window_states: Vec::from([false; 10]),

            textures,
            message_channel: mpsc::channel(),
            frame_times: History::new(0..120, 2.0),
            audio_stream: None,

            state,
        })
    }

    /// Create the screen texture.
    pub fn make_screen_texture(
        ctx: &Context,
        size: [usize; 2],
        filter: TextureOptions,
    ) -> TextureId {
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
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct State {
    /// A list of last opened ROMs. Size is capped to 10, last opened
    /// ROM is at index 0. The oldest ROM gets removed first.
    pub last_opened: Vec<PathBuf>,
    /// User configuration options.
    pub options: Options,
}

/// A message that can be sent from some async context.
pub enum Message {
    /// A file picked by the user to be opend as a ROM, from the "Open ROM" file
    /// picker dialog.
    FileOpen(File),
}

/// User-configurable options.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub sys: SystemConfig,
    /// Input configuration.
    pub input: Input,

    /// Fast forward speed for the hold button.
    pub fast_forward_hold_speed: usize,
    /// Fast forward speed for the toggle button.
    pub fast_forward_toggle_speed: usize,
    /// Enable rewinding.
    pub enable_rewind: bool,
    /// Rewind buffer size (if enabled), in seconds.
    pub rewind_buffer_size: usize,

    /// Texture filter applied to the display.
    pub tex_filter: TextureOptions,
    /// GUI mode.
    pub gui_style: GuiStyle,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sys: Default::default(),
            input: Input::new(),
            fast_forward_hold_speed: 2,
            fast_forward_toggle_speed: 2,
            enable_rewind: true,
            rewind_buffer_size: 10,
            tex_filter: TextureOptions::NEAREST,
            #[cfg(target_arch = "wasm32")]
            gui_style: GuiStyle::SingleWindow,
            #[cfg(not(target_arch = "wasm32"))]
            gui_style: GuiStyle::MultiWindow,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub enum GuiStyle {
    SingleWindow,
    MultiWindow,
}
