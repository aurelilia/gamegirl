use core::{
    common::System,
    debugger::Breakpoint,
    ggc::{
        cpu::{inst, DReg},
        io::{ppu, ppu::Ppu},
        GameGirl,
    },
    numutil::NumExt,
};
use std::{fmt::Write, iter};

use eframe::{
    egui::{
        vec2, Align, ColorImage, Context, ImageData, Label, RichText, ScrollArea, TextEdit,
        TextureFilter, TextureId, Ui,
    },
    epaint::ImageDelta,
};

use crate::{gui::App, Colour};

/// Debugger window with instruction view, stack inspection and register
/// inspection. Allows for inst-by-inst advancing.
pub fn debugger(gg: &mut GameGirl, ui: &mut Ui) {
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(150.0);
            let mut pc = gg.cpu.pc;
            let inst = inst::get_at(gg, pc);
            let arg = gg.read16(pc + 1);
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
                let arg = gg.read16(pc + 1);
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
                        RichText::new(format!("0x{:04X} - {:04X}", sp, gg.read16(sp))).monospace(),
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
pub fn breakpoints(gg: &mut GameGirl, ui: &mut Ui) {
    let bps = &mut gg.debugger.breakpoints;
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
        if !gg.options.rom_loaded {
            ui.label("No ROM loaded yet!");
            return;
        }

        let mut buf = String::with_capacity(100);
        for row_start in 0..0x1000 {
            let row_start = row_start * 0x10;
            write!(&mut buf, "{:04X} -", row_start).unwrap();
            for offset in 0..0x10 {
                write!(&mut buf, " {:02X}", gg.read(row_start + offset)).unwrap();
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
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

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

/// State for visual debugging tools.
#[derive(Default)]
pub struct VisualDebugState {
    /// Texture ID for VRAM viewer
    vram_texture: Option<TextureId>,
    /// Texture ID for BG viewer
    bg_texture: Option<TextureId>,
    /// Texture ID for Window viewer
    window_texture: Option<TextureId>,
}

/// Window showing current VRAM contents rendered as tiles.
pub(super) fn vram_viewer(app: &mut App, ctx: &Context, ui: &mut Ui) {
    let gg = app.gg.lock().unwrap();
    let gg = if let System::GGC(gg) = &*gg {
        gg
    } else {
        ui.label("Only available on GG/GGC!");
        return;
    };
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

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

    let img = upload_texture(ctx, 32, 24, &mut app.visual_debug.vram_texture, buf);
    ui.image(img, vec2(32. * 16., 24. * 16.));
}

/// Window showing 32x32 tile map of background and window.
pub(super) fn bg_map_viewer(app: &mut App, ctx: &Context, ui: &mut Ui) {
    fn render_tiles(
        ctx: &Context,
        gg: &GameGirl,
        window: bool,
        id: &mut Option<TextureId>,
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
        upload_texture(ctx, 32, 32, id, buf)
    }

    let gg = app.gg.lock().unwrap();
    let gg = if let System::GGC(gg) = &*gg {
        gg
    } else {
        ui.label("Only available on GG/GGC!");
        return;
    };
    if !gg.options.rom_loaded {
        ui.label("No ROM loaded yet!");
        return;
    }

    let bg_id = render_tiles(ctx, gg, false, &mut app.visual_debug.bg_texture);
    let win_id = render_tiles(ctx, gg, true, &mut app.visual_debug.window_texture);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label("Background Map");
            ui.image(bg_id, vec2(32. * 16., 32. * 16.));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label("Window Map");
            ui.image(win_id, vec2(32. * 16., 32. * 16.));
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
    id: &mut Option<TextureId>,
    buf: Vec<Colour>,
) -> TextureId {
    let id = get_or_make_texture(ctx, id);
    let img = ImageDelta::full(ImageData::Color(ColorImage {
        size: [x * 8, y * 8],
        pixels: buf,
    }));
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
fn get_or_make_texture(ctx: &Context, tex: &mut Option<TextureId>) -> TextureId {
    *tex.get_or_insert_with(|| App::make_screen_texture(ctx, [0, 0], TextureFilter::Nearest))
}
