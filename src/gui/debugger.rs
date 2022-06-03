use crate::numutil::NumExt;
use crate::system::cpu::{inst, DReg};
use crate::system::debugger::Breakpoint;
use crate::system::GameGirl;
use crate::Colour;
use eframe::egui::{Align, Label, RichText, ScrollArea, TextEdit, Ui};
use std::fmt::Write;

/// Debugger window with instruction view, stack inspection and register inspection.
/// Allows for inst-by-inst advancing.
pub fn debugger(gg: &mut GameGirl, ui: &mut Ui) {
    if !gg.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(150.0);
            let mut pc = gg.cpu.pc;
            let inst = inst::get_at(gg, pc);
            let arg = gg.mmu.read16(pc + 1);
            ui.add(
                Label::new(
                    RichText::new(format!("0x{:04X} {}", pc, inst.formatted_name(arg)))
                        .monospace()
                        .color(Colour::GREEN),
                )
                .wrap(false),
            );
            pc += inst.size().u16();
            for _ in 0..0xF {
                let inst = inst::get_at(gg, pc);
                let arg = gg.mmu.read16(pc + 1);
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:04X} {}", pc, inst.formatted_name(arg)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                pc += inst.size().u16();
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.add(Label::new(RichText::new("Stack:").monospace()).wrap(false));
            let mut sp = gg.cpu.sp;
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!("0x{:04X} - {:04X}", sp, gg.mmu.read16(sp)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                sp -= 2;
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in [DReg::AF, DReg::BC, DReg::DE, DReg::HL] {
                ui.monospace(format!("{:?} = {:04X}", reg, gg.cpu.dreg(reg)));
            }
            ui.monospace(format!("PC = {:04X}", gg.cpu.pc));
            ui.monospace(format!("SP = {:04X}", gg.cpu.sp));
            ui.add(
                Label::new(RichText::new(format!("HALT = {}", gg.cpu.halt)).monospace())
                    .wrap(false),
            );
            ui.add(
                Label::new(RichText::new(format!("IME = {}", gg.cpu.halt)).monospace()).wrap(false),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            gg.advance();
        }

        ui.checkbox(&mut gg.running, "Running");
    });
}

/// Window for configuring active breakpoints.
pub fn breakpoints(gg: &mut GameGirl, ui: &mut Ui) {
    let mut bps = gg.debugger.breakpoints.lock().unwrap();
    for bp in bps.iter_mut() {
        ui.horizontal(|ui| {
            ui.label("0x");
            if ui
                .add(TextEdit::singleline(&mut bp.addr_text).desired_width(40.0))
                .changed()
            {
                bp.addr = u16::from_str_radix(&bp.addr_text, 16).ok();
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
pub fn memory(gg: &mut GameGirl, ui: &mut Ui) {
    let mut buf = String::new();
    let mut position = None;

    ui.horizontal(|ui| {
        ui.label("Jump to ");
        if ui
            .add(TextEdit::singleline(&mut buf).desired_width(40.0))
            .changed()
        {
            position = u16::from_str_radix(&buf, 16).map(|a| a & 0xFF00).ok()
        }

        const POS: &[(&str, u16)] = &[
            ("Cart", 0),
            ("VRAM", 0x8000),
            ("CRAM", 0xA000),
            ("WRAM", 0xC000),
            ("OAM", 0xFE00),
            ("I/O", 0xFF00),
        ];
        for (name, pos) in POS.iter() {
            if ui.button(*name).clicked() {
                position = Some(*pos);
            }
        }
    });
    ui.separator();

    ScrollArea::vertical().show(ui, |ui| {
        if !gg.rom_loaded {
            ui.label("No ROM loaded yet!");
            return;
        }

        let mut buf = String::with_capacity(100);
        for row_start in 0..0x1000 {
            let row_start = row_start * 0x10;
            write!(&mut buf, "{:04X} -", row_start).unwrap();
            for offset in 0..0x10 {
                write!(&mut buf, " {:02X}", gg.mmu.read(row_start + offset)).unwrap();
            }

            let label = ui.add(Label::new(RichText::new(&buf).monospace()).wrap(false));
            if position == Some(row_start) {
                ui.scroll_to_rect(label.rect, Some(Align::Min));
            }
            buf.clear();
        }
    });
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(gg: &mut GameGirl, ui: &mut Ui) {
    if !gg.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.label(format!("Reported Title: {}", gg.mmu.cart.title(false)));
    ui.label(format!(
        "Reported Title (extended): {}",
        gg.mmu.cart.title(false)
    ));
    ui.label(format!("ROM banks: {}", gg.mmu.cart.rom_bank_count()));
    ui.label(format!("RAM banks: {}", gg.mmu.cart.ram_bank_count()));
    match () {
        _ if gg.mmu.cart.requires_cgb() => ui.label("GB Colour compatibility: Required"),
        _ if gg.mmu.cart.supports_cgb() => ui.label("GB Colour compatibility: Supported"),
        _ => ui.label("GB Colour compatibility: Unsupported"),
    };

    ui.separator();
    ui.label(format!("Current ROM0 bank: {}", gg.mmu.cart.rom0_bank));
    ui.label(format!("Current ROM1 bank: {}", gg.mmu.cart.rom1_bank));
    ui.label(format!("Current RAM bank: {}", gg.mmu.cart.ram_bank));
    ui.label(format!("MBC type and state: {:?}", gg.mmu.cart.kind));
}
