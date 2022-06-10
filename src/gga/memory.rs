pub const KB: usize = 1024;

pub const BIOS_START: usize = 0;
pub const BIOS_END: usize = 0x3FFF;
pub const WRAM1_START: usize = 0x02000000;
pub const WRAM1_END: usize = 0x0203FFFF;
pub const WRAM2_START: usize = 0x03000000;
pub const WRAM2_END: usize = 0x03007FFF;

pub struct Memory {
    pub bios: &'static [u8],
    pub wram1: [u8; 256 * KB],
    pub wram2: [u8; 32 * KB],
}
