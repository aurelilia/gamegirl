// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use eframe::egui::{Context, Ui};
use gamegirl::nds::Nds;

use super::Windows;
use crate::App;

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger ARM9").clicked();
    app.debugger_window_states[1] ^= ui.button("Debugger ARM7").clicked();
    app.debugger_window_states[2] ^= ui.button("Cartridge Viewer").clicked();
}

pub fn get_windows() -> Windows<Nds> {
    &[
        ("Debugger ARM9", |a, b, c, d| {
            super::armchair::debugger(&mut a.cpu9, b, c, d)
        }),
        ("Debugger ARM7", |a, b, c, d| {
            super::armchair::debugger(&mut a.cpu7, b, c, d)
        }),
        ("Cartridge", cart_info),
    ]
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(ds: &mut Nds, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.label(format!("{:#?}", ds.cart.header()));
}
