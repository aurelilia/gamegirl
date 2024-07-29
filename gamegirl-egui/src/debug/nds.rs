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
use gamegirl::nds::{Nds, NdsCpu};

use super::Windows;
use crate::{App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger ARM9").clicked();
    app.debugger_window_states[1] ^= ui.button("Debugger ARM7").clicked();
    app.debugger_window_states[2] ^= ui.button("Cartridge Viewer").clicked();
}

pub fn get_windows() -> Windows<Nds> {
    &[
        ("Debugger ARM9", debugger9),
        ("Debugger ARM7", debugger7),
        ("Cartridge", cart_info),
    ]
}

fn debugger7(ds: &mut Nds, ui: &mut Ui, a: &mut App, c: &Context) {
    let ds = &mut ds.nds7();
    debugger(ds, ui, a, c);
}
fn debugger9(ds: &mut Nds, ui: &mut Ui, a: &mut App, c: &Context) {
    let ds = &mut ds.nds9();
    debugger(ds, ui, a, c);
}

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
fn debugger(ds: &mut impl NdsCpu, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(300.0);
            // Account for prefetch
            let mut pc = ds.cpu().pc().wrapping_sub(ds.cpu().inst_size());
            ui.add(
                Label::new(
                    RichText::new(format!("0x{:08X} {}", pc, Nds::get_inst_mnemonic(ds, pc)))
                        .monospace()
                        .color(Colour::GREEN),
                )
                .extend(),
            );
            pc += ds.cpu().inst_size();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} {}", pc, Nds::get_inst_mnemonic(ds, pc)))
                            .monospace(),
                    )
                    .extend(),
                );
                pc += ds.cpu().inst_size();
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.add(Label::new(RichText::new("Stack:").monospace()).extend());
            let mut sp = ds.cpu().sp();
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
                ui.monospace(format!("R{:02} = {:08X}", reg, ds.cpu().reg(reg)));
            }
            ui.monospace(format!("SP  = {:08X}", ds.cpu().sp()));
            ui.monospace(format!("LR  = {:08X}", ds.cpu().lr()));
            ui.add(
                Label::new(RichText::new(format!("PC  = {:08X} ", ds.cpu().pc())).monospace())
                    .extend(),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.monospace("       NZCO                    IFT");
            ui.monospace(format!("CPSR = {:032b}", ds.cpu().cpsr));
            ui.monospace(format!("SPSR = {:032b}", ds.cpu().spsr()));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.monospace("            WSSGCCIII  GKDDDDSTTTTCHV");
            ui.monospace(format!("IF = {:032b}", ds.cpu().if_));
            ui.monospace(format!("IE = {:032b}", ds.cpu().ie));
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            ds.advance();
        }
        ui.checkbox(&mut ds.c.debugger.running, "Running");
        ui.checkbox(&mut ds.cpu().is_halted, "CPU Halted");

        if ds.cpu().ime {
            ui.label("(IME on)");
        }
    });

    super::debugger_footer(&mut ds.c.debugger, ui);
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(ds: &mut Nds, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.label(format!("{:#?}", ds.cart.header()));
}
