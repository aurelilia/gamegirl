// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use eframe::{
    egui::{Context, Event, TextureOptions},
    emath::History,
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
    CreationContext, Frame, Storage,
};
use egui_notify::{Anchor, Toasts};
use gamegirl::{
    common::Colour as RColour,
    frontend::{
        self,
        cpal::AudioStream,
        input::{Input, InputAction, InputSource},
        rewinder::{Rewinder, RewinderConfig},
    },
    Core, GameCart, Storage as GGStorage, SystemConfig,
};
use gilrs::{Axis, EventType, Gilrs};

use crate::{
    filter::{Blend, Filter, ScreenBuffer},
    gui::{self, cheat::CheatEngineState, options, APP_WINDOW_COUNT},
    input::{self, EguiKey, File},
    Colour,
};

#[derive(Copy, Clone, PartialEq)]
pub enum AxisState {
    Negative,
    Neutral,
    Positive,
}

impl AxisState {
    fn new(value: f32) -> Self {
        match value {
            ..=-0.5 => Self::Negative,
            0.5.. => Self::Positive,
            _ => Self::Neutral,
        }
    }
}

/// The main app struct used by the GUI.
pub struct App {
    /// The core currently running.
    pub core: Arc<Mutex<Box<dyn Core>>>,
    /// The path to the ROM currently running, if any. Always None on WASM.
    pub current_rom_path: Option<PathBuf>,

    /// Rewinder state.
    pub rewinder: Rewinder<10>,
    /// Screen buffer state.
    pub screen_buffer: ScreenBuffer,
    /// If the emulator is fast-forwarding using the toggle hotkey.
    pub fast_forward_toggled: bool,
    /// Dynamic loading state, to be used for debugging
    #[cfg(feature = "dynamic")]
    pub dyn_ctx: gamegirl::dynamic::DynamicContext,

    /// Texture(s) for the core's graphics output.
    pub textures: Vec<TextureId>,
    /// Game controller state
    pub gil: Gilrs,
    /// States for controller axes
    controller_axes: HashMap<Axis, AxisState>,
    /// Message channel for reacting to some async events, see [Message].
    pub message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
    /// Frame times.
    pub frame_times: History<f32>,
    /// Stream for audio.
    audio_stream: AudioStream,
    /// App window states.
    pub app_window_states: [bool; APP_WINDOW_COUNT],
    /// Debugger window states.
    pub debugger_window_states: Vec<bool>,
    /// Cheat engine state
    pub cheat: CheatEngineState,
    /// State of OSI
    pub on_screen_input: bool,
    /// State of options window
    pub open_option: options::Panel,
    /// Toasts
    pub toasts: Toasts,

    /// The App state, which is persisted on reboot.
    pub state: State,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        let size = self.update_gg(ctx);
        self.process_messages();
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
            let (img, filter) = self.screen_buffer.next_frame(
                size,
                pixels,
                self.state.options.tex_filter,
                self.state.options.screen_blend,
            );
            let img = ImageDelta::full(
                ImageData::Color(img), // todo meh
                filter,
            );
            let manager = ctx.tex_manager();
            manager.write().set(self.textures[0], img);
        }
        size
    }

    /// Process keyboard inputs and return the GG's next frame, if one was
    /// produced.
    fn get_frame(&mut self, ctx: &Context) -> (Option<Vec<RColour>>, [usize; 2]) {
        let raw_delta = ctx.input(|i| {
            for event in &i.events {
                if let Event::Key {
                    key,
                    pressed,
                    repeat: false,
                    ..
                } = event
                {
                    self.handle_evt(InputSource::Key((*key).into()), *pressed);
                }
            }
            while let Some(gilrs::Event { event, .. }) = self.gil.next_event() {
                match event {
                    EventType::ButtonPressed(b, _) => self.handle_evt(InputSource::Button(b), true),
                    EventType::ButtonReleased(b, _) => {
                        self.handle_evt(InputSource::Button(b), false)
                    }
                    EventType::AxisChanged(axis, value, _) => {
                        let prev = self
                            .controller_axes
                            .get(&axis)
                            .unwrap_or(&AxisState::Neutral);
                        let curr = AxisState::new(value);
                        if *prev != curr {
                            self.handle_evt(
                                InputSource::Axis {
                                    axis,
                                    is_neg: value < 0.0,
                                },
                                value.abs() > 0.5,
                            );
                        }
                        self.controller_axes.insert(axis, curr);
                    }
                    _ => (),
                }
            }
            i.unstable_dt
        });
        let delta = raw_delta.clamp(0.001, 0.016) - 0.0009;

        let mut core = self.core.lock().unwrap();
        let size = core.screen_size();

        if self.rewinder.rewinding {
            let frame = if let Some(state) = self.rewinder.rewind_buffer.pop() {
                core.load_state(state);
                core.c_mut().options.invert_audio_samples = true;
                core.produce_frame()
            } else {
                self.rewinder.rewinding = false;
                core.c_mut().options.invert_audio_samples = false;
                core.c_mut().video_buffer.pop()
            };
            (frame, size)
        } else {
            core.advance_delta(delta);
            let frame = core.c_mut().video_buffer.pop();
            if frame.is_some() && self.state.options.rewinder.enable_rewind {
                let state = core.save_state();
                self.rewinder.rewind_buffer.push(state);
            }
            (frame, size)
        }
    }

    fn handle_evt(&mut self, src: InputSource<EguiKey>, pressed: bool) {
        match self.state.options.input.key_triggered(src) {
            Some(InputAction::Button(btn)) => {
                let mut core = self.core.lock().unwrap();
                let time = core.get_time();
                core.c_mut().input.set(time, btn, pressed);
            }
            Some(InputAction::Hotkey(idx)) => input::HOTKEYS[idx as usize].1(self, pressed),
            None => (),
        }
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self) {
        while let Ok(msg) = self.message_channel.1.try_recv() {
            match msg {
                Message::RomOpen(file) => {
                    self.save_game();

                    // TODO This breaks the WASM build!!!
                    let save = GGStorage::load(file.path.clone(), "".into());
                    let sys = gamegirl::load_cart_maybe_zip(
                        GameCart {
                            rom: file.content,
                            save,
                        },
                        &self.state.options.sys,
                    );
                    match sys {
                        Ok(sys) => {
                            *self.core.lock().unwrap() = sys;
                        }
                        Err(e) => {
                            self.toasts
                                .error(format!("Error loading ROM: {e}"))
                                .duration(Some(Duration::from_secs(5)));
                            return;
                        }
                    }

                    self.audio_stream = frontend::cpal::setup(self.core.clone());

                    self.current_rom_path = file.path.clone();
                    if let Some(path) = file.path {
                        self.toasts
                            .success(format!("Loaded ROM: {path:?}"))
                            .duration(Some(Duration::from_secs(5)));

                        if let Some(existing) =
                            self.state.last_opened.iter().position(|p| *p == path)
                        {
                            self.state.last_opened.swap(0, existing);
                        } else {
                            self.state.last_opened.insert(0, path);
                            self.state.last_opened.truncate(10);
                        }
                    } else {
                        self.toasts
                            .success("Loaded ROM")
                            .duration(Some(Duration::from_secs(5)));
                    }
                }

                Message::ReplayOpen(file) => {
                    self.save_game();
                    let mut core = self.core.lock().unwrap();
                    core.reset();
                    core.c_mut().input.load_replay(file.content);
                    self.toasts
                        .info("Loaded replay")
                        .duration(Some(Duration::from_secs(5)));
                }

                Message::BiosOpen { file, console_id } => {
                    self.state
                        .options
                        .sys
                        .bioses
                        .iter_mut()
                        .find(|b| b.console_id == console_id)
                        .map(|b| b.bios = Some(file.content.clone()));
                }

                Message::Error(msg) => {
                    self.toasts
                        .error(msg)
                        .duration(Some(Duration::from_secs(5)));
                }

                #[cfg(feature = "dynamic")]
                Message::CoreLoad(path) => {
                    if let Ok(idx) = self
                        .dyn_ctx
                        .load_core(&path)
                        .inspect_err(|e| log::error!("Failed to load core! {e:#?}"))
                    {
                        let mut lock = self.core.lock().unwrap();
                        let old_core = mem::replace(&mut *lock, dummy_core());
                        let old_core = Box::leak(old_core);
                        let rom = old_core.get_rom();

                        let new_core = (self.dyn_ctx.get_core(idx).loader)(rom);
                        let vtable = ptr::metadata(new_core.as_ref() as *const _);

                        *lock = unsafe {
                            Box::from_raw(ptr::from_raw_parts::<dyn Core>(
                                old_core as *const _ as *const (),
                                vtable,
                            ) as *mut _)
                        };

                        self.toasts
                            .success(format!("Loaded core: {path:?}"))
                            .duration(Duration::from_secs(10));
                    }
                }
            }
        }
    }

    /// Save the system cart RAM, if a cart is loaded and it has RAM.
    pub fn save_game(&self) {
        if let Some(save) = self.core.lock().unwrap().make_save() {
            GGStorage::save(self.current_rom_path.clone(), save);
        }
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
            TextureOptions::NEAREST,
        )];

        let (tx, rx) = mpsc::channel();
        #[cfg(feature = "dynamic")]
        let tx2 = tx.clone();

        Box::new(App {
            core,
            current_rom_path: None,

            rewinder: Rewinder::new(state.options.rewinder.rewind_buffer_size),
            screen_buffer: ScreenBuffer::default(),
            fast_forward_toggled: false,
            #[cfg(feature = "dynamic")]
            dyn_ctx: gamegirl::dynamic::DynamicContext::watch_dir(move |path| {
                tx2.send(Message::CoreLoad(path)).unwrap();
            }),

            app_window_states: [false; APP_WINDOW_COUNT],
            debugger_window_states: Vec::from([false; 10]),
            cheat: CheatEngineState::default(),
            on_screen_input: false,
            open_option: options::Panel::About,
            toasts: Toasts::default().with_anchor(Anchor::BottomLeft),

            textures,
            gil: Gilrs::new().unwrap(),
            controller_axes: HashMap::with_capacity(6),
            message_channel: (tx, rx),
            frame_times: History::new(0..120, 2.0),
            audio_stream: AudioStream::empty(),

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
    RomOpen(File),
    /// A file picked by the user to be opened as a replay.
    ReplayOpen(File),
    /// An error occured.
    Error(String),
    /// A BIOS file was picked.
    BiosOpen { file: File, console_id: String },
    #[cfg(feature = "dynamic")]
    /// A new core got compiled and should be loaded.
    /// Only used when dynamic support is compiled in.
    CoreLoad(PathBuf),
}

/// User-configurable options.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub sys: SystemConfig,
    /// Input configuration.
    pub input: Input<EguiKey>,
    /// Rewind configuration.
    pub rewinder: RewinderConfig,

    /// Texture filter applied to the display.
    pub tex_filter: Filter,
    /// Blending applied to the display.
    pub screen_blend: Blend,
    /// Require pixel perfect scaling.
    pub pixel_perfect: bool,
    /// Always preserve aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// GUI mode.
    pub gui_style: GuiStyle,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            sys: Default::default(),
            input: Input::new(),
            rewinder: RewinderConfig::default(),
            tex_filter: Filter::Nearest,
            screen_blend: Blend::None,
            pixel_perfect: false,
            preserve_aspect_ratio: true,
            #[cfg(target_arch = "wasm32")]
            gui_style: GuiStyle::OnTop,
            #[cfg(not(target_arch = "wasm32"))]
            gui_style: GuiStyle::MultiWindow,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
pub enum GuiStyle {
    OnTop,
    AllWindows,
    MultiWindow,
}

impl App {
    pub fn pause(&mut self) {
        let mut core = self.core.lock().unwrap();
        let c = core.c_mut();
        c.debugger.running = !c.debugger.running;
        if c.debugger.running {
            self.toasts
                .info("Resuming")
                .duration(Some(Duration::from_secs(2)));
        } else {
            self.toasts
                .info("Paused")
                .duration(Some(Duration::from_secs(2)));
        }
    }

    pub fn reset(&mut self) {
        self.core.lock().unwrap().reset();
        self.toasts
            .warning("Console reset")
            .duration(Some(Duration::from_secs(5)));
    }
}
