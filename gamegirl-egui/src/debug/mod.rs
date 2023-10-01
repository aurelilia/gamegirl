// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod gga;
mod ggc;

use std::any::Any;

use common::Core;
use eframe::egui::{self, Context, Ui};
use gamegirl::{gga::GameGirlAdv, ggc::GameGirl};

use crate::app::App;

type Windows<T> = &'static [(&'static str, fn(&mut T, &mut Ui, &mut App, &Context))];

pub fn menu(app: &mut App, ui: &mut Ui) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = core.as_any();

    maybe_system::<GameGirl>(core, |_| ggc::ui_menu(app, ui));
    maybe_system::<GameGirlAdv>(core, |_| gga::ui_menu(app, ui));
}

pub fn render(app: &mut App, ctx: &Context) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = core.as_any();

    maybe_system::<GameGirl>(core, |c| render_inner(ggc::get_windows(), c, app, ctx));
    maybe_system::<GameGirlAdv>(core, |c| render_inner(gga::get_windows(), c, app, ctx));
}

fn render_inner<T: Core>(windows: Windows<T>, core: &mut T, app: &mut App, ctx: &Context) {
    let mut states = app.debugger_window_states.clone();
    for ((name, runner), state) in windows.iter().zip(states.iter_mut()) {
        egui::Window::new(*name)
            .open(state)
            .show(ctx, |ui| runner(core, ui, app, ctx));
    }
    app.debugger_window_states = states;
}

fn maybe_system<T: Core + 'static>(core: &mut dyn Any, mut apply: impl FnMut(&mut T)) {
    if let Some(sys) = core.downcast_mut() {
        apply(sys)
    }
}
