use crate::Colour;
use core::{
    debugger::Breakpoint,
    gga::{
        addr::{HALTCNT, IME},
        GameGirlAdv,
    },
    numutil::NumExt,
};
use eframe::egui::{Label, RichText, TextEdit, Ui};

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
pub fn debugger(gg: &mut GameGirlAdv, ui: &mut Ui) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(200.0);
            // Account for prefetch
            let mut pc = gg.cpu.pc.wrapping_sub(gg.cpu.inst_size());
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
                        RichText::new(format!("0x{:08X} - {:08X}", sp, gg.get_word(sp)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                sp = sp.wrapping_add(4);
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in 0..=12 {
                ui.monospace(format!("R{:02} = {:08X}", reg, gg.cpu.reg(reg)));
            }
            ui.monospace(format!("SP  = {:08X}", gg.cpu.sp()));
            ui.monospace(format!("LR  = {:08X}", gg.cpu.lr()));
            ui.add(
                Label::new(RichText::new(format!("PC  = {:08X} ", gg.cpu.pc)).monospace())
                    .wrap(false),
            );
        });
    });
    ui.separator();

    ui.monospace("       NZCO                    IFT");
    ui.monospace(format!("CPSR = {:032b}", gg.cpu.cpsr));
    ui.monospace(format!("SPSR = {:032b}", gg.cpu.spsr()));
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            gg.advance();
        }
        ui.checkbox(&mut gg.options.running, "Running");

        if gg[HALTCNT].is_bit(15) {
            ui.label("(CPU is halted)");
        }
        if gg[IME].is_bit(0) {
            ui.label("(IME on)");
        }
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
    ui.label(format!("Reported Title: {}", gg.cart.title()));
    ui.label(format!("Reported Game Code: AGB-{}", gg.cart.game_code()));
}
