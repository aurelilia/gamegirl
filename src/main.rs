use gamegirl::gui;
use gamegirl::system::GameGirl;
use std::env::args;
use std::fs;
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let gg = GameGirl::new(fs::read(args().nth(1).unwrap()).unwrap(), None);
    let gg = Arc::new(Mutex::new(gg));
    let _stream = gamegirl::setup_cpal(gg.clone());
    gui::start(gg);
}
