// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    fs, mem,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

use dynacore::{common::components::input_replay::InputReplay, gamegirl::dummy_core};
use eframe::{
    egui::{Color32, Context, TextureOptions},
    epaint::{ColorImage, ImageData, ImageDelta, TextureId},
    CreationContext, Frame,
};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, INotifyWatcher, RecursiveMode, Watcher,
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

    /// Texture(s) for the core's graphics output.
    pub textures: Vec<TextureId>,
    /// Message channel for reacting to some async events, see [Message].
    pub message_channel: (mpsc::Sender<Message>, mpsc::Receiver<Message>),
    /// App window states.
    pub app_window_states: [bool; APP_WINDOW_COUNT],
    /// Needs to be kept alive
    _watcher: INotifyWatcher,
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
        let size = self.cores[0].c.screen_size();
        for (i, core) in self.cores.iter_mut().enumerate() {
            core.c.advance_delta(0.2);
            let frame = core.c.last_frame().map(|p| unsafe { mem::transmute(p) });
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

                Message::ReplayOpen(_) => {}

                Message::CoreOpen(path) => {
                    if let Ok(core) = crate::load_core(path) {
                        self.cores.push(core);
                        self.textures.push(App::make_screen_texture(
                            &ctx,
                            [160, 144],
                            TextureOptions::NEAREST,
                        ));

                        if let Some(rom) = self.rom.as_ref() {
                            for core in &mut self.cores {
                                core.c = (core.loader)(rom.clone());
                            }
                        }
                        self.update_test_suites();
                    }
                }

                Message::CopyHashToClipboard(core) => {
                    self.cores[core].c.advance_delta(0.2);
                    let hash = TestSuite::screen_hash(&mut self.cores[core].c);
                    ctx.output_mut(|o| o.copied_text = format!("0x{hash:X}"));
                }
            }
        }
    }

    pub fn update_test_suites(&mut self) {
        for core in &mut self.cores {
            for suite in self.suites.iter().skip(core.suites.len()) {
                core.suites
                    .push(Arc::clone(&suite).run_on_core(core.loader))
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
        let mut _watcher = notify::recommended_watcher(move |res| match res {
            Ok(notify::Event {
                kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
                mut paths,
                ..
            }) => {
                tx.send(Message::CoreOpen(paths.pop().unwrap())).unwrap();
            }
            Ok(_) => (),
            Err(_) => panic!(),
        })
        .unwrap();
        _watcher
            .watch(Path::new("./dyn-cores"), RecursiveMode::Recursive)
            .unwrap();

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
                loader: dynacore::new_core,
                _library: None,
                name: "Built-in".to_string(),
            }],
            replay: None,
            rom: None,
            suites: vec![],

            textures,
            app_window_states: [true; APP_WINDOW_COUNT],
            message_channel,
            _watcher,
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
