// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::iter;

use common::{numutil::NumExt, Core};
use eframe::{
    egui::{load::SizedTexture, Context, Label, RichText, TextureOptions, Ui},
    epaint::{vec2, ColorImage, ImageData, ImageDelta, TextureId},
};
use gamegirl::nds::Nds;

use super::Windows;
use crate::{App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger").clicked();
    app.debugger_window_states[1] ^= ui.button("Cartridge Viewer").clicked();
}

pub fn get_windows() -> Windows<Nds> {
    &[("Debugger", debugger), ("Cartridge", cart_info)]
}

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
fn debugger(ds: &mut Nds, ui: &mut Ui, _: &mut App, _: &Context) {
    let mut ds = ds.nds9();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(300.0);
            // Account for prefetch
            let mut pc = ds.cpu9.pc().wrapping_sub(ds.cpu9.inst_size());
            ui.add(
                Label::new(
                    RichText::new(format!("0x{:08X} {}", pc, ds.get_inst9_mnemonic(pc)))
                        .monospace()
                        .color(Colour::GREEN),
                )
                .extend(),
            );
            pc += ds.cpu9.inst_size();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} {}", pc, ds.get_inst9_mnemonic(pc)))
                            .monospace(),
                    )
                    .extend(),
                );
                pc += ds.cpu9.inst_size();
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.add(Label::new(RichText::new("Stack:").monospace()).extend());
            let mut sp = ds.cpu9.sp();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} - {:08X}", sp, ds.get::<u32>(sp)))
                            .monospace(),
                    )
                    .extend(),
                );
                sp = sp.wrapping_add(4);
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in 0..=12 {
                ui.monospace(format!("R{:02} = {:08X}", reg, ds.cpu9.reg(reg)));
            }
            ui.monospace(format!("SP  = {:08X}", ds.cpu9.sp()));
            ui.monospace(format!("LR  = {:08X}", ds.cpu9.lr()));
            ui.add(
                Label::new(RichText::new(format!("PC  = {:08X} ", ds.cpu9.pc())).monospace())
                    .extend(),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.monospace("       NZCO                    IFT");
            ui.monospace(format!("CPSR = {:032b}", ds.cpu9.cpsr));
            ui.monospace(format!("SPSR = {:032b}", ds.cpu9.spsr()));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.monospace("       GKDDDDSTTTTCHV");
            ui.monospace(format!("IF = {:016b}", ds.cpu9.if_));
            ui.monospace(format!("IE = {:016b}", ds.cpu9.ie));
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            ds.advance();
        }
        ui.checkbox(&mut ds.c.debugger.running, "Running");
        ui.checkbox(&mut ds.cpu9.is_halted, "CPU Halted");

        if ds.cpu9.ime {
            ui.label("(IME on)");
        }
    });

    super::debugger_footer(&mut ds.c.debugger, ui);
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(ds: &mut Nds, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.label(format!("{:#?}", ds.cart.header()));
}
