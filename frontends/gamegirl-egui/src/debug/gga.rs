// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::iter;

use eframe::{
    egui::{load::SizedTexture, Context, TextureOptions, Ui},
    epaint::{vec2, ColorImage, ImageData, ImageDelta, TextureId},
};
use gamegirl::{
    common::{numutil::NumExt, Core},
    gga::{
        hw::timer::{self},
        ppu::registers::{Window, WindowCtrl},
        GameGirlAdv,
    },
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
    app.debugger_window_states[7] ^= ui.button("PPU Register Viewer").clicked();
    app.debugger_window_states[3] ^= ui.button("BG Tileset Viewer").clicked();
    app.debugger_window_states[4] ^= ui.button("OBJ Tileset Viewer").clicked();
    ui.separator();
    app.debugger_window_states[5] ^= ui.button("Timer Status").clicked();
    app.debugger_window_states[6] ^= ui.button("DMA Status").clicked();
}

pub fn get_windows() -> Windows<GameGirlAdv> {
    &[
        ("Debugger", |a, b, c, d| {
            super::armchair::debugger(&mut a.cpu, b, c, d)
        }),
        ("Cartridge", cart_info),
        ("Remote Debugger", remote_debugger),
        ("BG Tileset Viewer", bg_tileset_viewer),
        ("OBJ Tileset Viewer", obj_tileset_viewer),
        ("Timer Status", timer_status),
        ("DMA Status", dma_status),
        ("PPU Register Viewer", ppu_registers),
    ]
}

/// Window showing information about the loaded ROM/cart.
pub fn cart_info(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.label(format!("Reported Title: {}", gg.cart.title()));
    ui.label(format!("Reported Game Code: AGB-{}", gg.cart.game_code()));
    ui.label(format!("Detected Save Type: {:?}", gg.cart.save_type));

    let pure = gg.cpu.opt.table.analyses.iter().filter(|a| a.pure).count();
    ui.label(format!(
        "Function Purity: {}/{}",
        pure,
        gg.cpu.opt.table.analyses.len()
    ));
    let percent_native = gg.cpu.opt.jit_ctx.stats.native_instructions as f32
        / gg.cpu.opt.jit_ctx.stats.total_instructions as f32;
    ui.label(format!(
        "JIT nativity: {}/{} ({:.2}%)",
        gg.cpu.opt.jit_ctx.stats.native_instructions,
        gg.cpu.opt.jit_ctx.stats.total_instructions,
        percent_native * 100.0
    ));
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
        let cnt = gg.ppu.regs.bg_cnt[bg.us()];
        let tile_base_addr = cnt.character_base_block().us() * 0x4000;
        let bpp8 = cnt.palette_mode() as u32 == 1;

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

    let mode = gg.ppu.regs.dispcnt.bg_mode();
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

    let mode = gg.ppu.regs.dispcnt.bg_mode();
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
        let current = gg.timers.time_read(timer, gg.get_time());
        ui.label(format!("Current Value: 0x{current:04X}"));

        let ctrl = gg.timers.control[timer];
        ui.label(format!(
            "Scaler: F/{} ({})",
            timer::DIVS[ctrl.prescaler().us()],
            ctrl.prescaler()
        ));

        ui.label(format!("Enabled: {:?}", ctrl.enable()));
        ui.label(format!("IRQ Enabled: {:?}", ctrl.irq_en()));
        if timer != 0 {
            ui.label(format!("Count-Up Mode: {:?}", ctrl.count_up()));
        }
        if timer != 3 {
            ui.separator();
        }
    }
}

fn dma_status(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    for dma in 0..4 {
        ui.heading(format!("DMA {dma}"));
        let ctrl = gg.dma.channels[dma].ctrl;
        ui.label(format!(
            "SAD: 0x{:08X} ({:?})",
            gg.dma.channels[dma].sad,
            ctrl.src_addr()
        ));
        ui.label(format!(
            "DAD: 0x{:08X} ({:?})",
            gg.dma.channels[dma].dad,
            ctrl.dest_addr()
        ));

        ui.label(format!(
            "Enable: {:?}, IRQ: {:?}",
            ctrl.dma_en(),
            ctrl.irq_en()
        ));
        ui.label(format!(
            "Type: {}, Repeat: {:?}",
            if ctrl.is_32bit() { "Word" } else { "Halfword" },
            ctrl.repeat_en()
        ));
        ui.label(format!("Timing: {:?}", ctrl.timing()));
    }
}

/// Window showing PPU state.
fn ppu_registers(gg: &mut GameGirlAdv, ui: &mut Ui, _: &mut App, _: &Context) {
    let cnt = gg.ppu.regs.dispcnt;
    ui.collapsing("Display Control", |ui| {
        ui.label(format!("BG Mode: {:?}", cnt.bg_mode()));
        if cnt.bg_mode() as usize > 3 {
            ui.label(format!("Frame Select: {}", cnt.frame_select() as usize));
        }

        ui.checkbox(&mut cnt.hblank_oam_free(), "OAM during H-Blank");
        ui.checkbox(
            &mut (cnt.character_mapping_mode() as usize == 0),
            "Object 2D mapping",
        );
        ui.checkbox(&mut cnt.forced_blank_enable(), "Forced Blank");

        for bg in 0..4 {
            ui.checkbox(&mut cnt.bg_en().is_bit(bg), format!("BG{bg} Enable"));
        }
        ui.checkbox(&mut cnt.obj_en(), "OBJ Enable");
        ui.checkbox(&mut cnt.win0_en(), "WIN0 Enable");
        ui.checkbox(&mut cnt.win1_en(), "WIN1 Enable");
        ui.checkbox(&mut cnt.winobj_en(), "WINOBJ Enable");
    });

    for bg in 0..4 {
        let bgcnt = gg.ppu.regs.bg_cnt[bg];
        ui.collapsing(format!("BG{bg} Control"), |ui| {
            ui.label(format!("Priority: {}", bgcnt.priority()));
            ui.label(format!(
                "Character Base Block: 0x{:04X}",
                bgcnt.character_base_block().u32() * 0x4000
            ));
            ui.label(format!(
                "Screen Base Block: {:?}",
                bgcnt.screen_base_block().u32() * 0x800
            ));
            ui.checkbox(&mut bgcnt.mosaic_en(), "Mosaic");
            ui.checkbox(&mut (bgcnt.palette_mode() as usize == 1), "8BPP Mode");

            ui.separator();
            ui.label(format!("Scroll X: {}", gg.ppu.regs.bg_offsets[bg * 2]));
            ui.label(format!("Scroll Y: {}", gg.ppu.regs.bg_offsets[bg * 2 + 1]));
        });
    }

    ui.collapsing("Windows", |ui| {
        window_ui("0", &gg.ppu.regs.windows[0], ui);
        window_ui("1", &gg.ppu.regs.windows[1], ui);
        window_ctrl_ui("OBJ", &gg.ppu.regs.win_obj, ui);
        window_ctrl_ui("OUT", &gg.ppu.regs.win_out, ui);
    });
}

fn window_ui(win: &str, ctrl: &Window, ui: &mut Ui) {
    ui.label(format!(
        "Window {win} Left/Right: {}-{}",
        ctrl.left(),
        ctrl.right()
    ));
    ui.label(format!(
        "Window {win} Top/Bottom: {}-{}",
        ctrl.top(),
        ctrl.bottom()
    ));
    window_ctrl_ui(win, &ctrl.control, ui)
}

fn window_ctrl_ui(win: &str, ctrl: &WindowCtrl, ui: &mut Ui) {
    for bg in 0..4 {
        ui.checkbox(
            &mut ctrl.bg_en().is_bit(bg),
            format!("WIN{win} BG{bg} Enable"),
        );
    }
    ui.checkbox(&mut ctrl.obj_en(), "OBJ Enable");
    ui.checkbox(&mut ctrl.special_en(), "Special Enable");
}
