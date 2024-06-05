// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{components::debugger::Breakpoint, Core};
use eframe::egui::{Context, Label, RichText, TextEdit, Ui};
use gamegirl::psx::PlayStation;

use super::Windows;
use crate::{App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger").clicked();
    app.debugger_window_states[1] ^= ui.button("Breakpoints").clicked();
    app.debugger_window_states[2] ^= ui.button("ISO Viewer").clicked();
}

pub fn get_windows() -> Windows<PlayStation> {
    &[
        ("Debugger", debugger),
        ("Breakpoints", breakpoints),
        ("ISO", cart_info),
    ]
}

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
fn debugger(ps: &mut PlayStation, ui: &mut Ui, _: &mut App, _: &Context) {
    if !ps.options.rom_loaded {
        ui.label("No ISO loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(200.0);
            let mut pc = ps.cpu.pc;
            let inst = ps.get(pc);
            ui.add(
                Label::new(
                    RichText::new(format!("0x{:08X} {}", pc, PlayStation::get_mnemonic(inst)))
                        .monospace()
                        .color(Colour::GREEN),
                )
                .wrap(false),
            );
            pc += 4;
            for _ in 0..0x1F {
                let inst = ps.get(pc);
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} {}", pc, PlayStation::get_mnemonic(inst)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                pc += 4;
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in 0..32 {
                ui.monospace(format!("R{:02} = {:08X}", reg, ps.cpu.reg(reg)));
            }
            ui.monospace(format!("HI  = {:08X}", ps.cpu.hi));
            ui.monospace(format!("LO  = {:08X}", ps.cpu.lo));
            ui.add(
                Label::new(RichText::new(format!("PC  = {:08X} ", ps.cpu.pc)).monospace())
                    .wrap(false),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            ps.advance();
        }
        ui.checkbox(&mut ps.debugger.running, "Running");
    });

    ui.separator();
    super::inst_dump(ui, &mut ps.debugger);
}

/// Window for configuring active breakpoints.
pub fn breakpoints(ps: &mut PlayStation, ui: &mut Ui, _: &mut App, _: &Context) {
    for bp in ps.debugger.breakpoints.iter_mut() {
        ui.horizontal(|ui| {
            ui.label("0x");
            if ui
                .add(TextEdit::singleline(&mut bp.value_text).desired_width(80.0))
                .changed()
            {
                bp.value = u32::from_str_radix(&bp.value_text, 16).ok();
            }
            ui.checkbox(&mut bp.pc, "PC");
            ui.checkbox(&mut bp.write, "Write");
        });
    }

    ui.horizontal(|ui| {
        if ui.button("Add").clicked() {
            ps.debugger.breakpoints.push(Breakpoint::default());
        }
        if ui.button("Clear").clicked() {
            ps.debugger.breakpoints.clear();
        }
    });
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(ps: &mut PlayStation, ui: &mut Ui, _: &mut App, _: &Context) {
    if !ps.options.rom_loaded {
        ui.label("No ISO loaded yet!");
        return;
    }
    ui.label(format!("Reported Title: {}", ps.iso.title()));
}
