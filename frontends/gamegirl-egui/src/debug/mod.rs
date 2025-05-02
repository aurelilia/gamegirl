// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

mod armchair;
mod gga;
mod ggc;
mod nds;
// #[cfg(not(target_arch = "wasm32"))]
// mod psx;

use std::any::Any;

use eframe::egui::{
    self, Align, Color32, ComboBox, Context, Layout, RichText, ScrollArea, TextEdit, Ui,
};
use gamegirl::{
    common::common::debugger::{Breakpoint, Debugger, Severity},
    gga::GameGirlAdv,
    ggc::GameGirl,
    nds::Nds,
    Core,
};

use crate::app::{App, GuiStyle};

type DbgFn<T> = fn(&mut T, &mut Ui, &mut App, &Context);
type Windows<T> = &'static [(&'static str, DbgFn<T>)];

pub fn menu(app: &mut App, ui: &mut Ui) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = (&mut **core) as &mut dyn Any;

    maybe_system::<GameGirl>(core, |_| ggc::ui_menu(app, ui));
    maybe_system::<GameGirlAdv>(core, |_| gga::ui_menu(app, ui));
    maybe_system::<Nds>(core, |_| nds::ui_menu(app, ui));
    // #[cfg(not(target_arch = "wasm32"))]
    // maybe_system::<gamegirl::psx::PlayStation>(core, |_| psx::ui_menu(app,
    // ui));
}

pub fn render(app: &mut App, ctx: &Context) {
    let lock = app.core.clone();
    let mut core = lock.lock().unwrap();
    let core = (&mut **core) as &mut dyn Any;

    maybe_system::<GameGirl>(core, |c| render_inner(ggc::get_windows(), c, app, ctx));
    maybe_system::<GameGirlAdv>(core, |c| render_inner(gga::get_windows(), c, app, ctx));
    maybe_system::<Nds>(core, |c| render_inner(nds::get_windows(), c, app, ctx));
    // #[cfg(not(target_arch = "wasm32"))]
    // maybe_system::<gamegirl::psx::PlayStation>(core, |c| {
    //     render_inner(psx::get_windows(), c, app, ctx)
    // });
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
        GuiStyle::OnTop | GuiStyle::AllWindows => {
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

fn debugger_footer(dbg: &mut Debugger, ui: &mut Ui) {
    ui.add_space(10.0);
    inst_dump(ui, dbg);
    ui.add_space(10.0);
    breakpoints(dbg, ui);
    ui.add_space(10.0);
    event_log(dbg, ui);
}

fn inst_dump(ui: &mut Ui, debugger: &mut Debugger) {
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

fn breakpoints(dbg: &mut Debugger, ui: &mut Ui) {
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
                    bp.value = u32::from_str_radix(&bp.value_text, 16).ok();
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

fn event_log(dbg: &mut Debugger, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Event Log");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ComboBox::from_label("Level")
                .selected_text(format!("{:?}", dbg.diagnostic_level))
                .show_ui(ui, |ui| {
                    for level in [
                        Severity::Error,
                        Severity::Warning,
                        Severity::Info,
                        Severity::Debug,
                        Severity::None,
                    ] {
                        ui.selectable_value(
                            &mut dbg.diagnostic_level,
                            level,
                            format!("{:?}", level),
                        );
                    }
                });
            if ui.button("Clear").clicked() {
                dbg.diagnostic_events.clear();
            }
        });
    });

    ui.separator();
    ScrollArea::vertical().show(ui, |ui| {
        for event in dbg.diagnostic_events.iter().rev() {
            ui.label(RichText::new(&event.event).color(severity_color(event.severity)))
                .on_hover_ui(|ui| {
                    ui.label(format!("Type: {}", event.evt_type));
                    ui.label(format!("Time: {:?}", event.time));
                });
        }
    });
}

fn severity_color(severity: Severity) -> Color32 {
    match severity {
        Severity::Error => Color32::RED,
        Severity::Warning => Color32::YELLOW,
        Severity::Info => Color32::LIGHT_BLUE,
        Severity::Debug => Color32::LIGHT_GRAY,
        Severity::None => Color32::BLACK,
    }
}
