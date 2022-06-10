use super::memory::KB;

pub const PPU_REG_START: usize = 0x04000000;
pub const PPU_REG_END: usize = 0x04000055;
pub const PALETTE_START: usize = 0x05000000;
pub const PALETTE_END: usize = 0x050003FF;
pub const VRAM_START: usize = 0x06000000;
pub const VRAM_END: usize = 0x06017FFF;
pub const OAM_START: usize = 0x07000000;
pub const OAM_END: usize = 0x070003FF;

pub struct Ppu {
    pub regs: [u8; 56],
    pub palette: [u8; KB],
    pub vram: [u8; 96 * KB],
    pub oam: [u8; KB],
}
