// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod gga;
mod ggc;
#[cfg(feature = "psx")]
mod psx;

use std::any::Any;

use common::Core;
use eframe::egui::{self, Context, Ui};
use gamegirl::{gga::GameGirlAdv, ggc::GameGirl};

use crate::app::{App, GuiStyle};

type DbgFn<T> = fn(&mut T, &mut Ui, &mut App, &Context);
type Windows<T> = &'static [(&'static str, DbgFn<T>)];

pub fn menu(app: &mut App, ui: &mut Ui) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = core.as_any();

    maybe_system::<GameGirl>(core, |_| ggc::ui_menu(app, ui));
    maybe_system::<GameGirlAdv>(core, |_| gga::ui_menu(app, ui));
    #[cfg(feature = "psx")]
    maybe_system::<gamegirl::psx::PlayStation>(core, |_| psx::ui_menu(app, ui));
}

pub fn render(app: &mut App, ctx: &Context) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = core.as_any();

    maybe_system::<GameGirl>(core, |c| render_inner(ggc::get_windows(), c, app, ctx));
    maybe_system::<GameGirlAdv>(core, |c| render_inner(gga::get_windows(), c, app, ctx));
    #[cfg(feature = "psx")]
    maybe_system::<gamegirl::psx::PlayStation>(core, |c| {
        render_inner(psx::get_windows(), c, app, ctx)
    });
}

fn render_inner<T: Core>(windows: Windows<T>, core: &mut T, app: &mut App, ctx: &Context) {
    let mut states = app.debugger_window_states.clone();
    for ((name, runner), state) in windows.iter().zip(states.iter_mut()) {
        make_window(app, ctx, name, state, core, *runner);
    }
    app.debugger_window_states = states;
}

fn maybe_system<T: Core + 'static>(core: &mut dyn Any, mut apply: impl FnMut(&mut T)) {
    if let Some(sys) = core.downcast_mut() {
        apply(sys)
    }
}

fn make_window<T>(
    app: &mut App,
    ctx: &Context,
    title: &str,
    open: &mut bool,
    core: &mut T,
    content: DbgFn<T>,
) {
    match app.state.options.gui_style {
        GuiStyle::SingleWindow => {
            egui::Window::new(title)
                .open(open)
                .show(ctx, |ui| content(core, ui, app, ctx));
        }
        GuiStyle::MultiWindow => {
            if *open {
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of(title),
                    egui::ViewportBuilder::default().with_title(title),
                    |ctx, _| {
                        egui::CentralPanel::default().show(ctx, |ui| content(core, ui, app, ctx));
                        *open &= !ctx.input(|i| i.raw.viewport.close_requested);
                    },
                )
            }
        }
    }
}
