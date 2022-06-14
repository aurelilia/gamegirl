use serde::{Deserialize, Serialize};

pub const KB: usize = 1024;

pub const BIOS_START: usize = 0;
pub const BIOS_END: usize = 0x3FFF;
pub const WRAM1_START: usize = 0x02000000;
pub const WRAM1_END: usize = 0x0203FFFF;
pub const WRAM2_START: usize = 0x03000000;
pub const WRAM2_END: usize = 0x03007FFF;

#[derive(Deserialize, Serialize)]
pub struct Memory {
    #[serde(skip)]
    #[serde(default = "bios")]
    pub bios: &'static [u8],
    #[serde(with = "serde_arrays")]
    pub wram1: [u8; 256 * KB],
    #[serde(with = "serde_arrays")]
    pub wram2: [u8; 32 * KB],
}

fn bios() -> &'static [u8] {
    include_bytes!("bios.bin")
}
