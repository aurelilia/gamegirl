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

use common::{
    components::debugger::{Breakpoint, Debugger},
    numutil::NumExt,
    Core,
};
use eframe::egui::{self, Align, Color32, Context, Layout, TextEdit, Ui};
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
                        *open &= !ctx.input(|i| i.viewport().close_requested());
                    },
                )
            }
        }
    }
}

fn debugger_footer<T: NumExt>(dbg: &mut Debugger<T>, ui: &mut Ui) {
    ui.add_space(10.0);
    inst_dump(ui, dbg);
    ui.add_space(10.0);
    breakpoints(dbg, ui);
}

fn inst_dump<T: NumExt>(ui: &mut Ui, debugger: &mut Debugger<T>) {
    ui.horizontal(|ui| {
        ui.heading("CPU Logging");
        if let Some(string) = debugger.traced_instructions.as_ref() {
            ui.label(format!("Buffer: {:03}MB", string.len() / 1_000_000));
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Dump").clicked() && debugger.traced_instructions.is_some() {
                std::fs::write(
                    "instruction-dump",
                    debugger.traced_instructions.as_ref().unwrap().as_bytes(),
                )
                .unwrap();
                debugger.traced_instructions = None;
            }
            if ui.button("Start").clicked() {
                debugger.traced_instructions = Some(String::with_capacity(10_000_000));
            }
        });
    });
}

fn breakpoints<T: NumExt>(dbg: &mut Debugger<T>, ui: &mut Ui) {
    let bps = &mut dbg.breakpoints;
    ui.horizontal(|ui| {
        ui.heading("Breakpoints");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Clear").clicked() {
                bps.clear();
            }
            if ui.button("Add").clicked() {
                bps.push(Breakpoint::default());
            }
        });
    });
    ui.indent(2412, |ui| {
        let mut del = None;
        for (i, bp) in bps.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("0x");
                if ui
                    .add(TextEdit::singleline(&mut bp.value_text).desired_width(40.0))
                    .changed()
                {
                    let value = u32::from_str_radix(&bp.value_text, 16).ok();
                    bp.value = value.map(T::from_u32);
                }
                ui.checkbox(&mut bp.pc, "PC");
                ui.checkbox(&mut bp.write, "Write");

                if Some(i) == dbg.breakpoint_hit {
                    ui.colored_label(Color32::RED, "Hit!");
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Delete").clicked() {
                        del = Some(i);
                    }
                });
            });
        }

        if let Some(i) = del {
            bps.remove(i);
        }
    });
}
