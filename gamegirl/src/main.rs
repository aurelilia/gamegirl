use core::common::System;
use std::sync::{Arc, Mutex};

use gamegirl::gui;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let gg = System::default();
    let gg = Arc::new(Mutex::new(gg));
    let _stream = gamegirl::setup_cpal(gg.clone());
    gui::start(gg);
}
