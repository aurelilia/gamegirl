// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{future::Future, path::PathBuf, sync::mpsc};

use rfd::FileHandle;

use crate::app::Message;

/// A file picked by the user.
pub struct File {
    /// File content in bytes
    pub content: Vec<u8>,
    /// Path of the file. Always None on WASM.
    pub path: Option<PathBuf>,
}

/// Open a file dialog. This operation is async and returns immediately,
/// sending a [Message] once the user has picked a file.
pub fn open_rom(sender: mpsc::Sender<Message>) {
    let task = rfd::AsyncFileDialog::new()
        .add_filter("GameGirl games", &["gb", "gbc", "gba", "elf", "iso"])
        .pick_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            let path = path(&file);
            let content = file.read().await;
            sender.send(Message::RomOpen(File { content, path })).ok();
        }
    });
}

/// Open a file dialog. This operation is async and returns immediately,
/// sending a [Message] once the user has picked a file.
pub fn open_replay(sender: mpsc::Sender<Message>) {
    let task = rfd::AsyncFileDialog::new()
        .add_filter("GameGirl replays", &["rpl"])
        .pick_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            let path = path(&file);
            let content = file.read().await;
            sender
                .send(Message::ReplayOpen(File { content, path }))
                .ok();
        }
    });
}

/// Open a file dialog. This operation is async and returns immediately,
/// sending a [Message] once the user has picked a file.
pub fn open_core(sender: mpsc::Sender<Message>) {
    let task = rfd::AsyncFileDialog::new()
        .add_filter("GameGirl cores", &["so"])
        .pick_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            let path = path(&file);
            sender.send(Message::CoreOpen(path.unwrap())).ok();
        }
    });
}

fn path(f: &FileHandle) -> Option<PathBuf> {
    Some(f.path().to_path_buf())
}

fn execute<F: Future<Output = ()> + Send + 'static>(f: F) {
    std::thread::spawn(move || futures_executor::block_on(f));
}