use crate::debugger::Breakpoint;
use crate::gga::GameGirlAdv;
use crate::Colour;
use eframe::egui::{Label, RichText, TextEdit, Ui};

/// Debugger window with instruction view, stack inspection and register inspection.
/// Allows for inst-by-inst advancing.
pub fn debugger(gg: &mut GameGirlAdv, ui: &mut Ui) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(150.0);
            let mut pc = gg.cpu.pc;
            ui.add(
                Label::new(
                    RichText::new(format!("0x{:08X} {}", pc, gg.get_inst_mnemonic(pc)))
                        .monospace()
                        .color(Colour::GREEN),
                )
                .wrap(false),
            );
            pc += gg.cpu.inst_size();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} {}", pc, gg.get_inst_mnemonic(pc)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                pc += gg.cpu.inst_size();
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.add(Label::new(RichText::new("Stack:").monospace()).wrap(false));
            let mut sp = gg.cpu.sp();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:08X} - {:08X}", sp, gg.read_word(sp)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                sp -= 4;
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in 0..=12 {
                ui.monospace(format!("R{reg} = {:08X}", gg.cpu.reg(reg)));
            }
            ui.monospace(format!("SP = {:08X}", gg.cpu.sp()));
            ui.monospace(format!("LR = {:08X}", gg.cpu.lr()));
            ui.monospace(format!("PC = {:08X}", gg.cpu.pc));
            ui.monospace(format!("CPSR = {:32b}", gg.cpu.cpsr));
            ui.monospace(format!("SPSR = {:32b}", gg.cpu.spsr()));
            ui.add(
                Label::new(RichText::new(format!("HALT = {}", gg.cpu.halt)).monospace())
                    .wrap(false),
            );
            ui.add(
                Label::new(RichText::new(format!("IME = {}", gg.cpu.ime)).monospace()).wrap(false),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            gg.advance();
        }

        ui.checkbox(&mut gg.options.running, "Running");
    });
}

/// Window for configuring active breakpoints.
pub fn breakpoints(gg: &mut GameGirlAdv, ui: &mut Ui) {
    let mut bps = gg.debugger.breakpoints.lock().unwrap();
    for bp in bps.iter_mut() {
        ui.horizontal(|ui| {
            ui.label("0x");
            if ui
                .add(TextEdit::singleline(&mut bp.addr_text).desired_width(80.0))
                .changed()
            {
                bp.addr = u32::from_str_radix(&bp.addr_text, 16).ok();
            }
            ui.checkbox(&mut bp.pc, "PC");
            ui.checkbox(&mut bp.write, "Write");
        });
    }

    ui.horizontal(|ui| {
        if ui.button("Add").clicked() {
            bps.push(Breakpoint::default());
        }
        if ui.button("Clear").clicked() {
            bps.clear();
        }
    });
}

/// Memory viewer showing the entire GG's address space.
pub fn memory(_gg: &mut GameGirlAdv, ui: &mut Ui) {
    ui.label("On a GGA? Good luck with rendering that.");
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(gg: &mut GameGirlAdv, ui: &mut Ui) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }
    ui.label("This is a ROM for sure!");
}
