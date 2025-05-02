use egui::{Context, Label, RichText, Ui};
use gamegirl::gga::armchair::{interface::Bus, state::Register, Address, Cpu};

use crate::{App, Colour};

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
pub fn debugger<S: Bus>(cpu: &mut Cpu<S>, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(600.0);
            // Account for prefetch
            let mut pc = cpu.state.pc() - cpu.state.next_instruction_offset();
            let inst = cpu.bus.get(&mut cpu.state, pc);
            ui.add(
                Label::new(
                    RichText::new(format!(
                        "0x{:08X} {}",
                        pc.0,
                        cpu.state.get_inst_mnemonic(inst)
                    ))
                    .monospace()
                    .color(Colour::GREEN),
                )
                .extend(),
            );
            pc += cpu.state.next_instruction_offset();
            for _ in 0..0xF {
                let inst = cpu.bus.get(&mut cpu.state, pc);
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "0x{:08X} {}",
                            pc.0,
                            cpu.state.get_inst_mnemonic(inst)
                        ))
                        .monospace(),
                    )
                    .extend(),
                );
                pc += cpu.state.next_instruction_offset();
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.add(Label::new(RichText::new("Stack:").monospace()).extend());
            let mut sp = cpu.state.sp();
            for _ in 0..0xF {
                ui.add(
                    Label::new(
                        RichText::new(format!(
                            "{sp} - {:08X}",
                            cpu.bus.get::<u32>(&mut cpu.state, sp)
                        ))
                        .monospace(),
                    )
                    .extend(),
                );
                sp += Address::WORD;
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in 0..=12 {
                ui.monospace(format!("R{:02} = {:08X}", reg, cpu.state[Register(reg)]));
            }
            ui.monospace(format!("SP  = {:08X}", cpu.state.sp().0));
            ui.monospace(format!("LR  = {:08X}", cpu.state.lr().0));
            ui.add(
                Label::new(RichText::new(format!("PC  = {:08X} ", cpu.state.pc().0)).monospace())
                    .extend(),
            );
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.monospace("       NZCO                    IFT");
            ui.monospace(format!("CPSR = {:032b}", cpu.state.cpsr()));
            ui.monospace(format!("SPSR = {:032b}", cpu.state.spsr()));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.monospace("       GKDDDDSTTTTCHV");
            ui.monospace(format!("IF = {:016b}", cpu.state.intr.if_));
            ui.monospace(format!("IE = {:016b}", cpu.state.intr.ie));
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            cpu.continue_running();
        }
        ui.checkbox(&mut cpu.bus.debugger().running, "Running");
        ui.checkbox(&mut cpu.state.is_halted, "CPU Halted");

        if cpu.state.intr.ime {
            ui.label("(IME on)");
        }
    });

    super::debugger_footer(&mut cpu.bus.debugger(), ui);
}
