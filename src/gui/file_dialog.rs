use crate::gui::Message;
use std::future::Future;
use std::sync::mpsc;
use std::thread;

pub fn open(sender: mpsc::Sender<Message>) {
    let task = rfd::AsyncFileDialog::new()
        .add_filter("GameGirl games", &["gb", "gbc"])
        .pick_file();

    execute(async move {
        let file = task.await;
        if let Some(file) = file {
            let file = file.read().await;
            sender.send(Message::FileOpen(file)).ok();
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn execute<F: Future<Output = ()> + Send + 'static>(f: F) {
    thread::spawn(move || futures_executor::block_on(f));
}

#[cfg(target_arch = "wasm32")]
fn execute<F: Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}
