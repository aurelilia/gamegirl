use core::common::System;
use gamegirl::gui;
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let gg = System::default();
    let gg = Arc::new(Mutex::new(gg));
    let _stream = gamegirl::setup_cpal(gg.clone());
    gui::start(gg);
}