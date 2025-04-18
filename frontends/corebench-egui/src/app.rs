// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{
    fs,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Instant,
};

use eframe::{
    egui::{Color32, Context, OutputCommand, TextureOptions},
    emath::History,
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
    CreationContext, Frame,
};
use gamegirl::{
    common::common::input::{InputReplay, ReplayState},
    dummy_core,
    dynamic::DynamicContext,
};

use crate::{
    gui::{self, file_dialog::File, APP_WINDOW_COUNT},
    testsuite::TestSuite,
    DCore,
};

/// The main app struct used by the GUI.
pub struct App {
    /// Cores currently loaded into the workbench.
    pub cores: Vec<DCore>,
    /// Replay currently loaded into the workbench.
    pub replay: Option<InputReplay>,
    /// ROM currently loaded into the workbench.
    pub rom: Option<Vec<u8>>,
    /// Test suites currently loaded into the workbench.
    pub suites: Vec<Arc<TestSuite>>,
    /// Toggle for benchmark graph.
    pub bench_iso: bool,

    /// Texture(s) for the core's graphics output.
    pub textures: Vec<TextureId>,
    /// Message channel for reacting to some async events, see [Message].
    pub message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
    /// App window states.
    pub app_window_states: [bool; APP_WINDOW_COUNT],
    /// Dynamic loading
    pub dyn_ctx: DynamicContext,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.process_messages(ctx);
        let size = self.update_frames(ctx);
        gui::draw(self, ctx, size);

        // Immediately repaint, since the GG will have a new frame.
        // egui will automatically bind the framerate to VSYNC.
        ctx.request_repaint();
    }
}

impl App {
    fn update_frames(&mut self, ctx: &Context) -> [usize; 2] {
        let now = ctx.input(|i| i.time);
        let size = self.cores[0].c.screen_size();
        for (i, core) in self.cores.iter_mut().enumerate() {
            let time = Instant::now();
            core.c.advance_delta(0.05);
            let elapsed = time.elapsed().as_micros() as f64;
            core.bench.add(now, elapsed / 1000.0);

            let frame = core.c.c_mut().video_buffer.pop_recent().map(|p| {
                p.into_iter()
                    .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
                    .collect::<Vec<_>>()
            });
            if let Some(pixels) = frame {
                let img = ImageDelta::full(
                    ImageData::Color(ColorImage { size, pixels }.into()),
                    TextureOptions::NEAREST,
                );
                let manager = ctx.tex_manager();
                manager.write().set(self.textures[i], img);
            }
        }
        size
    }

    /// Process all async messages that came in during this frame.
    fn process_messages(&mut self, ctx: &Context) {
        while let Ok(msg) = self.message_channel.1.try_recv() {
            match msg {
                Message::RomOpen(file) => {
                    for core in &mut self.cores {
                        core.c = (core.loader)(file.content.clone());
                    }
                    self.rom = Some(file.content.clone());
                }

                Message::ReplayOpen(file) => {
                    let replay = InputReplay::load(String::from_utf8(file.content).unwrap());
                    self.replay = Some(replay.clone());
                    for core in &mut self.cores {
                        core.c.c_mut().input.replay = ReplayState::Playback(replay.clone());
                    }
                }

                Message::CoreOpen(path) => {
                    if let Ok(core) = crate::load_core(&mut self.dyn_ctx, path) {
                        self.cores.push(core);
                        self.textures.push(App::make_screen_texture(
                            ctx,
                            [160, 144],
                            TextureOptions::NEAREST,
                        ));

                        if let Some(rom) = self.rom.as_ref() {
                            for core in &mut self.cores {
                                core.c = (core.loader)(rom.clone());
                            }
                        }
                        if let Some(replay) = self.replay.as_ref() {
                            for core in &mut self.cores {
                                core.c.c_mut().input.replay = ReplayState::Playback(replay.clone());
                            }
                        }
                        self.update_test_suites();
                    }
                }

                Message::CopyHashToClipboard(core) => {
                    self.cores[core].c.advance_delta(0.2);
                    let hash = TestSuite::screen_hash(&mut self.cores[core].c);
                    ctx.output_mut(|o| {
                        o.commands
                            .push(OutputCommand::CopyText(format!("0x{hash:X}")))
                    });
                }
            }
        }
    }

    pub fn update_test_suites(&mut self) {
        for core in &mut self.cores {
            for suite in self.suites.iter().skip(core.suites.len()) {
                core.suites.push(Arc::clone(suite).run_on_core(core.loader))
            }
        }
    }

    pub fn new(ctx: &CreationContext<'_>) -> Box<Self> {
        let textures = vec![App::make_screen_texture(
            &ctx.egui_ctx,
            [160, 144],
            TextureOptions::NEAREST,
        )];
        let message_channel = mpsc::channel();

        let tx = message_channel.0.clone();
        let dyn_ctx = DynamicContext::watch_dir(move |path| {
            tx.send(Message::CoreOpen(path)).unwrap();
        });
        for file in fs::read_dir("./dyn-cores").unwrap() {
            let file = file.unwrap();
            message_channel
                .0
                .send(Message::CoreOpen(file.path()))
                .unwrap();
        }

        Box::new(App {
            cores: vec![DCore {
                c: dummy_core(),
                suites: vec![],
                bench: History::new(10..5000, 30.0),
                bench_iso: Arc::new(Mutex::new(History::new(10..5000, 100.0))),
                loader: gamegirl::dynamic::new_core,
                idx: None,
                name: "Baseline".to_string(),
            }],
            replay: None,
            rom: None,
            suites: vec![],
            bench_iso: false,

            textures,
            app_window_states: [true; APP_WINDOW_COUNT],
            message_channel,
            dyn_ctx,
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
            ColorImage::new(size, Color32::BLACK).into(),
            filter,
        );
        id
    }
}

/// A message that can be sent from some async context.
pub enum Message {
    /// A file picked by the user to be opend as a ROM, from the "Open ROM" file
    /// picker dialog.
    RomOpen(File),
    /// A file picked by the user to be opened as a replay.
    ReplayOpen(File),
    /// A file picked by the user to be opened as a core.
    CoreOpen(PathBuf),
    /// Copy the hash of a core's screen to the clipboard.
    CopyHashToClipboard(usize),
}
