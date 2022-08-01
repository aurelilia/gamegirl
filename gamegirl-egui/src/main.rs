// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::{Arc, Mutex};

use gamegirl::System;
use gamegirl_egui::gui;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    let gg = System::default();
    let gg = Arc::new(Mutex::new(gg));
    let _stream = gamegirl_egui::setup_cpal(gg.clone());
    gui::start(gg);
}
