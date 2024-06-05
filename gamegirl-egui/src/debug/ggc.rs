// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{fmt::Write, iter};

use common::{numutil::NumExt, Core};
use eframe::{
    egui::{
        load::SizedTexture, vec2, Align, ColorImage, Context, ImageData, Label, RichText,
        ScrollArea, TextureId, TextureOptions, Ui,
    },
    epaint::ImageDelta,
};
use gamegirl::ggc::{
    cpu::{inst, DReg},
    io::{
        addr::{IE, IF, VRAM_SELECT, WRAM_SELECT},
        ppu::{self, Ppu},
    },
    GameGirl,
};

use super::Windows;
use crate::{app::App, Colour};

pub fn ui_menu(app: &mut App, ui: &mut eframe::egui::Ui) {
    app.debugger_window_states[0] ^= ui.button("Debugger").clicked();
    app.debugger_window_states[1] ^= ui.button("Memory Viewer").clicked();
    app.debugger_window_states[2] ^= ui.button("Cartridge Viewer").clicked();
    ui.separator();
    app.debugger_window_states[3] ^= ui.button("VRAM Viewer").clicked();
    app.debugger_window_states[4] ^= ui.button("Background Map Viewer").clicked();
}

pub fn get_windows() -> Windows<GameGirl> {
    &[
        ("Debugger", debugger),
        ("Memory", memory),
        ("Cartridge", cart_info),
        ("VRAM Viewer", vram_viewer),
        ("Background Map Viewer", bg_map_viewer),
    ]
}

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
fn debugger(gg: &mut GameGirl, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(150.0);
            let mut pc = gg.cpu.pc;
            let inst = inst::get_at(gg, pc);
            let arg = gg.get::<u16>(pc + 1);
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
                let arg = gg.get::<u16>(pc + 1);
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
                        RichText::new(format!("0x{:04X} - {:04X}", sp, gg.get::<u16>(sp)))
                            .monospace(),
                    )
                    .wrap(false),
                );
                sp = sp.wrapping_add(2);
            }
        });
        ui.separator();

        ui.vertical(|ui| {
            for reg in [DReg::AF, DReg::BC, DReg::DE, DReg::HL] {
                ui.monospace(format!("{:?} = {:04X}", reg, gg.cpu.dreg(reg)));
            }
            ui.monospace(format!("PC = {:04X}", gg.cpu.pc));
            ui.monospace(format!("SP = {:04X}", gg.cpu.sp));

            ui.separator();
            ui.add(
                Label::new(RichText::new(format!("IME = {}", gg.cpu.ime as u8)).monospace())
                    .wrap(false),
            );
            ui.monospace(format!("IF = {:05b}", gg[IF] & 0x1F));
            ui.monospace(format!("IE = {:05b}", gg[IE] & 0x1F));

            if gg.cgb {
                ui.separator();
                ui.monospace(format!("VRAM = {}", gg[VRAM_SELECT] & 0x1));
                ui.monospace(format!("WRAM = {}", gg[WRAM_SELECT] & 0x7));
            }
        });
    });
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Advance").clicked() {
            gg.advance();
        }

        ui.checkbox(&mut gg.debugger.running, "Running");
    });

    super::debugger_footer(&mut gg.debugger, ui);
}

/// Memory viewer showing the entire GG's address space.
fn memory(gg: &mut GameGirl, ui: &mut Ui, _: &mut App, _: &Context) {
    let mut position = None;

    ui.horizontal(|ui| {
        ui.label("Jump to:");
        const POS: &[(&str, u16)] = &[
            ("Cart", 0),
            ("VRAM", 0x8000),
            ("CRAM", 0xA000),
            ("WRAM", 0xC000),
            ("OAM", 0xFE00),
            ("I/O", 0xFF00),
            ("ZRAM", 0xFF80),
        ];
        for (name, pos) in POS.iter() {
            if ui.button(*name).clicked() {
                position = Some(*pos);
            }
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("   "); // Padding
        ui.monospace("     0  1  2  3  4  5  6  7  8  9  A  B  C  D  E  F");
    });
    ScrollArea::vertical().show(ui, |ui| {
        if !gg.options.rom_loaded {
            ui.label("No ROM loaded yet!");
            return;
        }

        let mut buf = String::with_capacity(100);
        for row_start in 0..0x1000 {
            let row_start = row_start * 0x10;
            write!(&mut buf, "{:04X} -", row_start).unwrap();
            for offset in 0..0x10 {
                write!(&mut buf, " {:02X}", gg.get::<u8>(row_start + offset)).unwrap();
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
fn cart_info(gg: &mut GameGirl, ui: &mut Ui, _: &mut App, _: &Context) {
    ui.label(format!("Reported Title: {}", gg.cart.title(false)));
    ui.label(format!(
        "Reported Title (extended): {}",
        gg.cart.title(false)
    ));
    ui.label(format!("ROM banks: {}", gg.cart.rom_bank_count()));
    ui.label(format!("RAM banks: {}", gg.cart.ram_bank_count()));
    match () {
        _ if gg.cart.requires_cgb() => ui.label("GB Colour compatibility: Required"),
        _ if gg.cart.supports_cgb() => ui.label("GB Colour compatibility: Supported"),
        _ => ui.label("GB Colour compatibility: Unsupported"),
    };

    ui.separator();
    ui.label(format!("Current ROM0 bank: {}", gg.cart.rom0_bank));
    ui.label(format!("Current ROM1 bank: {}", gg.cart.rom1_bank));
    ui.label(format!("Current RAM bank: {}", gg.cart.ram_bank));
    ui.label(format!("MBC type and state: {:?}", gg.cart.kind));
}

/// Texture ID for VRAM viewer
const VRAM_TEX: usize = 1;
/// Texture ID for BG viewer
const BG_TEX: usize = 2;
/// Texture ID for Window viewer
const WIN_TEX: usize = 3;

/// Window showing current VRAM contents rendered as tiles.
fn vram_viewer(gg: &mut GameGirl, ui: &mut Ui, app: &mut App, ctx: &Context) {
    let mut buf = make_buffer(32, 24);
    for tile in (0..0x1800).step_by(0x10) {
        let tile_idx = tile / 0x10;
        // Tile in Bank 0
        draw_tile(
            gg,
            &mut buf,
            (tile_idx & 0xF).u8(),
            (tile_idx / 0x10).u8(),
            tile,
        );
        // Tile in Bank 1 (for CGB)
        draw_tile(
            gg,
            &mut buf,
            (tile_idx & 0xF).u8() + 0x10,
            (tile_idx / 0x10).u8(),
            tile + 0x2000,
        );
    }

    let img = upload_texture(ctx, 32, 24, app, VRAM_TEX, buf, TextureOptions::NEAREST);
    ui.image(Into::<SizedTexture>::into((
        img,
        vec2(32. * 16., 24. * 16.),
    )));
}

/// Window showing 32x32 tile map of background and window.
fn bg_map_viewer(gg: &mut GameGirl, ui: &mut Ui, app: &mut App, ctx: &Context) {
    fn render_tiles(
        ctx: &Context,
        gg: &GameGirl,
        app: &mut App,
        window: bool,
        id: usize,
    ) -> TextureId {
        let mut buf = make_buffer(32, 32);
        for tile_idx_addr in 0..(32 * 32) {
            let data_addr = Ppu::bg_idx_tile_data_addr(gg, window, tile_idx_addr);
            draw_tile(
                gg,
                &mut buf,
                (tile_idx_addr % 0x20).u8(),
                (tile_idx_addr / 0x20).u8(),
                data_addr,
            );
        }
        upload_texture(ctx, 32, 32, app, id, buf, TextureOptions::NEAREST)
    }

    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    let bg_id = render_tiles(ctx, gg, app, false, BG_TEX);
    let win_id = render_tiles(ctx, gg, app, true, WIN_TEX);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("Background Map");
            ui.image(Into::<SizedTexture>::into((
                bg_id,
                vec2(32. * 16., 32. * 16.),
            )));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label("Window Map");
            ui.image(Into::<SizedTexture>::into((
                win_id,
                vec2(32. * 16., 32. * 16.),
            )));
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
fn draw_tile(gg: &GameGirl, buf: &mut [Colour], x: u8, y: u8, tile_ptr: u16) {
    for line in 0..8 {
        let base_addr = tile_ptr + (line * 2);
        let high = gg.mem.vram[base_addr.us()];
        let low = gg.mem.vram[base_addr.us() + 1];

        for pixel in 0..8 {
            let colour_idx = (high.bit(7 - pixel) << 1) + low.bit(7 - pixel);
            let colour = ppu::COLOURS[colour_idx.us()];

            let idx = ((x.us() * 8) + pixel.us()) + (((y.us() * 8) + line.us()) * 256);
            buf[idx] = Colour::from_gray(colour);
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
