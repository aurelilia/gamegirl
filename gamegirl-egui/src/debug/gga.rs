// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::iter;

use common::{numutil::NumExt, Core};
use eframe::{
    egui::{load::SizedTexture, Context, Label, RichText, TextureOptions, Ui},
    epaint::{vec2, ColorImage, ImageData, ImageDelta, TextureId},
};
use gamegirl::gga::{
    addr::{BG0CNT, DISPCNT, IE, IF, IME, TM0CNT_H},
    timer::{self, Timers},
    GameGirlAdv,
};

use super::Windows;
use crate::{App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger").clicked();
    app.debugger_window_states[1] ^= ui.button("Cartridge Viewer").clicked();
    if cfg!(all(feature = "remote-debugger", target_family = "unix")) {
        app.debugger_window_states[2] ^= ui.button("Remote Debugger").clicked();
    }
    ui.separator();
    app.debugger_window_states[3] ^= ui.button("BG Tileset Viewer").clicked();
    app.debugger_window_states[4] ^= ui.button("OBJ Tileset Viewer").clicked();
    ui.separator();
    app.debugger_window_states[5] ^= ui.button("Timer Status").clicked();
}

pub fn get_windows() -> Windows<GameGirlAdv> {
    &[
        ("Debugger", debugger),
        ("Cartridge", cart_info),
        ("Remote Debugger", remote_debugger),
        ("BG Tileset Viewer", bg_tileset_viewer),
        ("OBJ Tileset Viewer", obj_tileset_viewer),
        ("Timer Status", timer_status),
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
            ui.set_min_width(300.0);
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

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.monospace("       NZCO                    IFT");
            ui.monospace(format!("CPSR = {:032b}", gg.cpu.cpsr));
            ui.monospace(format!("SPSR = {:032b}", gg.cpu.spsr()));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.monospace("       GKDDDDSTTTTCHV");
            ui.monospace(format!("IF = {:016b}", gg[IF]));
            ui.monospace(format!("IE = {:016b}", gg[IE]));
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            gg.advance();
        }
        ui.checkbox(&mut gg.debugger.running, "Running");
        ui.checkbox(&mut gg.cpu.is_halted, "CPU Halted");

        if gg[IME].is_bit(0) {
            ui.label("(IME on)");
        }
    });

    super::debugger_footer(&mut gg.debugger, ui);
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
#[cfg(all(feature = "remote-debugger", target_family = "unix"))]
fn remote_debugger(_: &mut GameGirlAdv, ui: &mut Ui, app: &mut App, _: &Context) {
    use std::sync::{Arc, RwLock};

    use gamegirl::remote_debugger::DebuggerStatus;
    use once_cell::sync::Lazy;

    static DBG: Lazy<Arc<RwLock<DebuggerStatus>>> = Lazy::new(Arc::default);

    fn launch_debugger(app: &App) {
        let gg = app.core.clone();
        let path = app.current_rom_path.clone().unwrap();
        std::thread::spawn(|| gamegirl::remote_debugger::init(gg, path, DBG.clone()));
    }

    let stat = *DBG.read().unwrap();
    match stat {
        DebuggerStatus::NotActive => {
            ui.label("Remote debugger is not active.");
            if ui.button("Launch Server").clicked() {
                launch_debugger(app);
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
                launch_debugger(app);
            }
        }
    }
}

#[cfg(not(all(feature = "remote-debugger", target_family = "unix")))]
pub(super) fn remote_debugger(_: &mut GameGirlAdv, _: &mut Ui, _: &mut App, _: &Context) {}

/// Texture IDs for BG VRAM viewer
const BG_VRAM_TEX: [usize; 4] = [1, 2, 3, 4];
/// Texture IDs for OBJ VRAM viewer
const OBJ_VRAM_TEX: [usize; 2] = [4, 5];

/// Window showing current tilesets for all background layers.
fn bg_tileset_viewer(gg: &mut GameGirlAdv, ui: &mut Ui, app: &mut App, ctx: &Context) {
    fn draw_bg_layer(gg: &GameGirlAdv, bg: u32, ui: &mut Ui, app: &mut App, ctx: &Context) {
        let mut buf = make_buffer(32, 32);
        let cnt = gg[BG0CNT + bg * 2];
        let tile_base_addr = cnt.bits(2, 2).us() * 0x4000;
        let bpp8 = cnt.is_bit(7);

        for tile_idx_addr in 0..(32 * 32) {
            let data_addr = tile_base_addr + (tile_idx_addr * if bpp8 { 8 * 8 } else { 8 * 4 });
            draw_tile(
                gg,
                &mut buf,
                (tile_idx_addr % 0x20) as u8,
                (tile_idx_addr / 0x20) as u8,
                data_addr.u32(),
                bpp8,
            );
        }

        let tex = upload_texture(
            ctx,
            32,
            32,
            app,
            BG_VRAM_TEX[bg.us()],
            buf,
            TextureOptions::NEAREST,
        );
        ui.vertical(|ui| {
            ui.label(format!(
                "Tileset for BG{bg} ({}bpp)",
                if bpp8 { "8" } else { "4" }
            ));
            ui.image(Into::<SizedTexture>::into((
                tex,
                vec2(32. * 16., 32. * 16.),
            )));
        });
    }

    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    let mode = gg.ppu.dispcnt.bg_mode();
    ui.label(format!("Current PPU mode: {mode:?}"));
    if (mode as usize) < 3 {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                draw_bg_layer(gg, 0, ui, app, ctx);
                ui.separator();
                draw_bg_layer(gg, 1, ui, app, ctx);
            });
            if (mode as usize) < 2 {
                ui.separator();
                ui.horizontal(|ui| {
                    draw_bg_layer(gg, 2, ui, app, ctx);
                    if (mode as usize) == 0 {
                        ui.separator();
                        draw_bg_layer(gg, 3, ui, app, ctx);
                    }
                });
            }
        });
    } else {
        ui.label("(Tileset viewer is not supported in bitmap modes)");
    }
}

/// Window showing current object tiles in 4bpp and 8bpp.
fn obj_tileset_viewer(gg: &mut GameGirlAdv, ui: &mut Ui, app: &mut App, ctx: &Context) {
    fn draw_set_layer(
        gg: &GameGirlAdv,
        tilemode: bool,
        bpp8: bool,
        ui: &mut Ui,
        app: &mut App,
        ctx: &Context,
    ) {
        let buffer_height = if tilemode { 32 } else { 16 };
        let mut buf = make_buffer(32, buffer_height);
        let tile_base_addr = if tilemode { 0x10000 } else { 0x14000 };
        for tile_idx_addr in 0..(32 * buffer_height) {
            let data_addr = tile_base_addr + (tile_idx_addr * if bpp8 { 8 * 8 } else { 8 * 4 });
            draw_tile(
                gg,
                &mut buf,
                (tile_idx_addr % 0x20) as u8,
                (tile_idx_addr / 0x20) as u8,
                data_addr.u32(),
                bpp8,
            );
        }

        let tex = upload_texture(
            ctx,
            32,
            buffer_height,
            app,
            OBJ_VRAM_TEX[bpp8 as usize],
            buf,
            TextureOptions::NEAREST,
        );
        ui.vertical(|ui| {
            ui.label(format!("Tileset in {}bpp", if bpp8 { "8" } else { "4" }));
            ui.image(Into::<SizedTexture>::into((
                tex,
                vec2(32. * 16., buffer_height as f32 * 16.),
            )));
        });
    }

    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    let mode = gg.ppu.dispcnt.bg_mode();
    ui.label(format!("Current PPU mode: {mode:?}"));
    ui.separator();
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            draw_set_layer(gg, (mode as usize) < 3, false, ui, app, ctx);
            ui.separator();
            draw_set_layer(gg, (mode as usize) < 3, true, ui, app, ctx);
        });
    });
}
/// Create a buffer with the given size in tiles (8x8 tiles)
fn make_buffer(x: usize, y: usize) -> Vec<Colour> {
    let count = (x * 8) * (y * 8);
    let mut buf = Vec::with_capacity(count);
    buf.extend(iter::repeat(Colour::BLACK).take(count));
    buf
}

/// Create the given texture if needed and then upload the given image to it.
fn upload_texture(
    ctx: &Context,
    x: usize,
    y: usize,
    app: &mut App,
    id: usize,
    buf: Vec<Colour>,
    filter: TextureOptions,
) -> TextureId {
    let id = get_or_make_texture(ctx, app, id);
    let img = ImageDelta::full(
        ImageData::Color(
            ColorImage {
                size: [x * 8, y * 8],
                pixels: buf,
            }
            .into(), // TODO meh. Arc is a kinda annoying here
        ),
        filter,
    );
    let manager = ctx.tex_manager();
    manager.write().set(id, img);
    id
}

/// Draw a full 8x8 tile to the given buffer. The pointer is in VRAM; X/Y is in
/// tiles.
fn draw_tile(gg: &GameGirlAdv, buf: &mut [Colour], x: u8, y: u8, tile_ptr: u32, is_8bpp: bool) {
    let ppu = &gg.ppu;
    for line in 0..8 {
        if is_8bpp {
            let base_addr = tile_ptr + (line * 8);
            for pixel in 0..8 {
                let idx = ((x.us() * 8) + pixel.us()) + (((y.us() * 8) + line.us()) * 256);
                let l = ppu.vram[(base_addr + pixel).us()];
                buf[idx] = Colour::from_rgb((l & 0xF) << 4, ((l >> 2) & 0xF) << 4, (l >> 4) << 4);
            }
        } else {
            let base_addr = tile_ptr + (line * 4);
            for idx in 0..4 {
                let byte = ppu.vram[(base_addr + idx).us()];
                let idx = ((x.us() * 8) + idx.us() * 2) + (((y.us() * 8) + line.us()) * 256);
                buf[idx] = Colour::from_gray((byte & 0xF) << 4);
                buf[idx + 1] = Colour::from_gray(byte & 0xF0);
            }
        }
    }
}

/// Get or create the given texture ID.
fn get_or_make_texture(ctx: &Context, app: &mut App, id: usize) -> TextureId {
    while app.textures.len() <= id {
        app.textures.push(App::make_screen_texture(
            ctx,
            [0, 0],
            TextureOptions::NEAREST,
        ));
    }
    app.textures[id]
}

fn timer_status(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    for timer in 0..4 {
        ui.heading(format!("Timer {timer}"));
        let current = match timer {
            0 => Timers::time_read::<0>(gg),
            1 => Timers::time_read::<1>(gg),
            2 => Timers::time_read::<2>(gg),
            _ => Timers::time_read::<3>(gg),
        };
        ui.label(format!("Current Value: 0x{current:04X}"));

        let ctrl = gg[TM0CNT_H + ((timer as u32) << 2)];
        ui.label(format!(
            "Scaler: F/{} ({})",
            timer::DIVS[(ctrl & 3).us()],
            ctrl & 3
        ));

        ui.label(format!("Enabled: {:?}", ctrl.is_bit(7)));
        ui.label(format!("IRQ Enabled: {:?}", ctrl.is_bit(6)));
        if timer != 0 {
            ui.label(format!("Count-Up Mode: {:?}", ctrl.is_bit(2)));
        }
        if timer != 3 {
            ui.separator();
        }
    }
}
