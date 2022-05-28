use crate::gui::Colour;
use crate::numutil::NumExt;
use crate::system::cpu::Interrupt;
use crate::system::io::addr::*;
use crate::GameGirl;

mod cgb;
mod dmg;

const BG_EN: u16 = 0;
const OBJ_EN: u16 = 1;
const BIG_OBJS: u16 = 2;
const BG_MAP: u16 = 3;
const ALT_BG_TILE: u16 = 4;
const WIN_EN: u16 = 5;
const WIN_MAP: u16 = 6;
const DISP_EN: u16 = 7;

pub struct Ppu {
    mode: Mode,
    mode_clock: u16,
    bg_occupied_pixels: [bool; 160 * 144],
    window_line: u8,
    kind: PpuKind,

    pub(crate) pixels: [Colour; 160 * 144],
}

impl Ppu {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        if !gg.lcdc(DISP_EN) {
            return;
        }
        let mode = {
            let ppu = gg.ppu();
            ppu.mode_clock += t_cycles as u16;
            if ppu.mode_clock < ppu.mode.cycles() {
                return;
            }
            ppu.mode_clock -= ppu.mode.cycles();
            ppu.mode
        };

        let next_mode = match mode {
            Mode::OAMScan => Mode::Upload,

            Mode::Upload => {
                Self::render_line(gg);
                Self::stat_interrupt(gg, 3);
                Mode::HBlank
            }

            Mode::HBlank => {
                gg.mmu[LY] += 1;
                Self::stat_interrupt(gg, 5);
                Self::lyc_interrupt(gg);
                if gg.mmu[LY] == 144 {
                    Self::stat_interrupt(gg, 4);
                    gg.request_interrupt(Interrupt::VBlank);
                    Mode::VBlank
                } else {
                    Mode::OAMScan
                }
            }

            Mode::VBlank => {
                gg.mmu[LY] += 1;
                Self::lyc_interrupt(gg);
                if gg.mmu[LY] > 153 {
                    gg.mmu[LY] = 0;
                    gg.mmu.ppu.window_line = 0;
                    gg.mmu.ppu.bg_occupied_pixels = [false; 160 * 144];
                    Self::stat_interrupt(gg, 5);
                    Mode::OAMScan
                } else {
                    Mode::VBlank
                }
            }
        };

        gg.mmu[STAT] =
            gg.mmu[STAT].set_bit(2, gg.mmu[LYC] == gg.mmu[LY]).u8() & 0xFC | next_mode.ordinal();
        gg.ppu().mode = next_mode;
    }

    fn stat_interrupt(gg: &mut GameGirl, bit: u16) {
        if gg.mmu[STAT].is_bit(bit) {
            gg.request_interrupt(Interrupt::Stat);
        }
    }

    fn lyc_interrupt(gg: &mut GameGirl) {
        if gg.mmu[LYC] == gg.mmu[LY] {
            Self::stat_interrupt(gg, 6);
        }
    }

    fn render_line(gg: &mut GameGirl) {
        match gg.mmu.ppu.kind {
            PpuKind::Dmg { .. } if gg.lcdc(BG_EN) => {
                Self::render_bg(gg);
                if gg.lcdc(WIN_EN) {
                    Self::render_window(gg);
                }
            }
            PpuKind::Dmg { .. } => Self::clear_line(gg),

            PpuKind::Cgb => {
                Self::render_bg(gg);
                if gg.lcdc(WIN_EN) {
                    Self::render_window(gg);
                }
            }
        }

        if gg.lcdc(OBJ_EN) {
            Self::render_objs(gg);
        }
    }

    fn render_bg(gg: &mut GameGirl) {
        // Only render until the point where the window starts, should it be active
        let end_x =
            if gg.lcdc(WIN_EN) && (7u8..166u8).contains(&gg.mmu[WX]) && gg.mmu[WY] <= gg.mmu[LY] {
                gg.mmu[WX] - 7
            } else {
                160
            };
        Self::render_bg_or_window(
            gg,
            gg.mmu[SCX],
            0,
            end_x,
            gg.map_addr(BG_MAP),
            gg.mmu[SCY].wrapping_add(gg.mmu[LY]),
            true,
        )
    }

    fn render_window(gg: &mut GameGirl) {
        let wx = gg.mmu[WX] as i16 - 7;
        if wx < 0 || wx > 159 || gg.mmu[WY] > gg.mmu[LY] {
            return;
        }

        Self::render_bg_or_window(
            gg,
            0,
            wx as u8,
            160,
            gg.map_addr(WIN_MAP),
            gg.mmu.ppu.window_line,
            false,
        );
        gg.ppu().window_line += 1;
    }

    fn render_bg_or_window(
        gg: &mut GameGirl,
        scroll_x: u8,
        start_x: u8,
        end_x: u8,
        map_addr: u16,
        map_line: u8,
        correct_tile_addr: bool,
    ) {
        let line = gg.mmu[LY];
        let mut tile_x = scroll_x & 7;
        let tile_y = map_line & 7;
        let mut tile_addr = map_addr + ((map_line / 8).u16() * 0x20) + (scroll_x >> 3).u16();
        let mut tile_data_addr =
            Self::bg_tile_data_addr(gg, gg.mmu.vram[tile_addr.us()].u16()) + (tile_y.u16() * 2);
        let mut high = gg.mmu.vram[tile_data_addr.us() + 1];
        let mut low = gg.mmu.vram[tile_data_addr.us()];

        for tile_idx_addr in start_x..end_x {
            let colour_idx = (high.bit(7 - tile_x.u16()) << 1) + low.bit(7 - tile_x.u16());
            // TODO set occupied
            let colour = Self::get_bg_colour(gg, colour_idx);
            gg.ppu().set_pixel(tile_idx_addr, line, colour);

            tile_x += 1;
            if tile_x == 8 {
                tile_x = 0;
                tile_addr = if correct_tile_addr && (tile_addr & 0x1F) == 0x1F {
                    tile_addr - 0x19
                } else {
                    tile_addr + 1
                };
                tile_data_addr = Self::bg_tile_data_addr(gg, gg.mmu.vram[tile_addr.us()].u16())
                    + (tile_y.u16() * 2);
                high = gg.mmu.vram[tile_data_addr.us() + 1];
                low = gg.mmu.vram[tile_data_addr.us()];
            }
        }
    }

    fn render_objs(gg: &mut GameGirl) {}

    fn set_pixel(&mut self, x: u8, y: u8, col: Colour) {
        let idx = x.us() + (y.us() * 160);
        self.pixels[idx] = col;
    }

    fn bg_idx_tile_data_addr(gg: &GameGirl, window: bool, idx: u16) -> u16 {
        let addr = match () {
            _ if window && gg.lcdc(WIN_MAP) => 0x1C00,
            _ if window => 0x1800,
            _ if gg.lcdc(BG_MAP) => 0x1C00,
            _ => 0x1800,
        } + idx;
        Self::bg_tile_data_addr(gg, gg.mmu.vram[addr.us()].u16())
    }

    fn bg_tile_data_addr(gg: &GameGirl, idx: u16) -> u16 {
        if gg.lcdc(ALT_BG_TILE) {
            idx * 0x10
        } else {
            0x1000 + (idx * 0x10)
        }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            mode: Mode::OAMScan,
            mode_clock: 0,
            bg_occupied_pixels: [false; 160 * 144],
            window_line: 0,
            kind: PpuKind::Dmg {
                used_x_obj_coords: [None; 10],
            },
            pixels: [Colour::BLACK; 160 * 144],
        }
    }
}

#[derive(Copy, Clone)]
pub enum PpuKind {
    Dmg { used_x_obj_coords: [Option<u8>; 10] },
    Cgb,
}

#[derive(Copy, Clone)]
enum Mode {
    HBlank = 204,
    VBlank = 456,
    OAMScan = 80,
    Upload = 172,
}

impl Mode {
    fn cycles(self) -> u16 {
        self as u16
    }

    fn ordinal(self) -> u8 {
        // ehhh
        match self {
            Mode::HBlank => 0,
            Mode::VBlank => 1,
            Mode::OAMScan => 2,
            Mode::Upload => 3,
        }
    }
}

impl GameGirl {
    #[inline]
    fn ppu(&mut self) -> &mut Ppu {
        &mut self.mmu.ppu
    }

    #[inline]
    fn lcdc(&self, bit: u16) -> bool {
        self.mmu[LCDC].is_bit(bit)
    }

    #[inline]
    fn map_addr(&self, bit: u16) -> u16 {
        if self.mmu[LCDC].is_bit(bit) {
            0x1C00
        } else {
            0x1800
        }
    }
}
