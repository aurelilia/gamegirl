// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

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
        .set_title("Open ROM")
        .add_filter(
            "GameGirl games",
            &["gb", "gbc", "gba", "nds", "elf", "iso", "zip"],
        )
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
        .set_title("Open Replay")
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
pub fn open_bios(sender: mpsc::Sender<Message>, console_id: String) {
    let task = rfd::AsyncFileDialog::new()
        .set_title("Open BIOS")
        .pick_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            let path = path(&file);
            let content = file.read().await;
            sender
                .send(Message::BiosOpen {
                    file: File { content, path },
                    console_id,
                })
                .ok();
        }
    });
}

/// Open a file save dialog. This operation is async and returns immediately.
pub fn save_replay(content: String) {
    let task = rfd::AsyncFileDialog::new()
        .set_title("Save Replay")
        .add_filter("GameGirl replays", &["rpl"])
        .save_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            file.write(content.as_bytes()).await.unwrap();
        }
    });
}

/// Open a file save dialog. This operation is async and returns immediately.
pub fn save_gamesave(name: String, content: Vec<u8>) {
    let task = rfd::AsyncFileDialog::new()
        .set_title("Save Game As")
        .set_file_name(format!("{name}.sav"))
        .add_filter("Game Save", &["sav"])
        .save_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            file.write(&content).await.unwrap();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn path(f: &FileHandle) -> Option<PathBuf> {
    Some(f.path().to_path_buf())
}

#[cfg(target_arch = "wasm32")]
fn path(_f: &FileHandle) -> Option<PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn execute<F: Future<Output = ()> + Send + 'static>(f: F) {
    std::thread::spawn(move || futures_executor::block_on(f));
}

#[cfg(target_arch = "wasm32")]
fn execute<F: Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}
