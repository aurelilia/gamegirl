// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::{components::debugger::Breakpoint, numutil::NumExt, Core};
use eframe::egui::{Context, Label, RichText, TextEdit, Ui};
use gamegirl::gga::{addr::IME, GameGirlAdv};

use super::Windows;
use crate::{App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] |= ui.button("Debugger").clicked();
    app.debugger_window_states[1] |= ui.button("Breakpoints").clicked();
    app.debugger_window_states[2] |= ui.button("Cartridge Viewer").clicked();

    if cfg!(feature = "remote-debugger") {
        app.debugger_window_states[3] |= ui.button("Remote Debugger").clicked();
    }
}

pub fn get_windows() -> Windows<GameGirlAdv> {
    &[
        ("Debugger", debugger),
        ("Breakpoints", breakpoints),
        ("Cartridge", cart_info),
        ("Remote Debugger", remote_debugger),
    ]
}

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
fn debugger(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(200.0);
            // Account for prefetch
            let mut pc = gg.cpu.pc().wrapping_sub(gg.cpu.inst_size());
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
                Label::new(RichText::new(format!("PC  = {:08X} ", gg.cpu.pc())).monospace())
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

        if gg[IME].is_bit(0) {
            ui.label("(IME on)");
        }
    });
}

/// Window for configuring active breakpoints.
pub fn breakpoints(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    for bp in gg.debugger.breakpoints.iter_mut() {
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
            gg.debugger.breakpoints.push(Breakpoint::default());
        }
        if ui.button("Clear").clicked() {
            gg.debugger.breakpoints.clear();
        }
    });
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }
    ui.label(format!("Reported Title: {}", gg.cart.title()));
    ui.label(format!("Reported Game Code: AGB-{}", gg.cart.game_code()));
    ui.label(format!("Detected Save Type: {:?}", gg.cart.save_type));
}

/// Window showing status of the remote debugger.
#[cfg(feature = "remote-debugger")]
pub(super) fn remote_debugger(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    {
        let gg = app.gg.lock().unwrap();
        if !matches!(&*gg, gamegirl::System::GGA(_)) {
            ui.label("Only available on GGA!");
            return;
        }
    }

    use gamegirl::remote_debugger::DebuggerStatus;
    let stat = *app.remote_dbg.read().unwrap();
    match stat {
        DebuggerStatus::NotActive => {
            ui.label("Remote debugger is not active.");
            if ui.button("Launch Server").clicked() {
                launch_debugger(app)
            }
        }
        DebuggerStatus::WaitingForConnection => {
            ui.label("Server running at localhost:17633");
            ui.label("Awaiting connection, if you are using lldb:");
            ui.monospace("> platform select remote-gdb-server");
            ui.monospace("> platform connect connect://localhost:17633");
            ui.label("If you are using gdb:");
            ui.monospace("> target remote localhost:17633");
        }
        DebuggerStatus::Running(addr) => {
            ui.label("Remote debugger is running.");
            ui.label(format!("Client address: {addr}"));
        }
        DebuggerStatus::Disconnected => {
            ui.label("Remote debugger disconnected/exited.");
            if ui.button("Relaunch Server").clicked() {
                launch_debugger(app)
            }
        }
    }
}

#[cfg(feature = "remote-debugger")]
fn launch_debugger(app: &mut App) {
    let gg = app.gg.clone();
    let path = app.current_rom_path.clone().unwrap();
    let remote = app.remote_dbg.clone();
    std::thread::spawn(|| gamegirl::remote_debugger::init(gg, path, remote));
}

#[cfg(not(feature = "remote-debugger"))]
pub(super) fn remote_debugger(_: &mut GameGirlAdv, _: &mut Ui, _: &mut App, _: &Context) {}
