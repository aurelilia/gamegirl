// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::Arc;

use eframe::egui::{Context, Ui};

use crate::{app::App, tests::SUITES};

pub(super) fn suites(app: &mut App, _ctx: &Context, ui: &mut Ui) {
    ui.label("Currently loaded suites:");
    for suite in &app.suites {
        ui.horizontal(|ui| {
            ui.label(&suite.name);
        });
    }

    ui.separator();
    ui.label("Add suites:");
    for suite in SUITES {
        if ui.button(suite.0).clicked() {
            app.suites.push(Arc::new(suite.1()));
            app.update_test_suites();
        }
    }
}
