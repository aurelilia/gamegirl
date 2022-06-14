use serde::{Deserialize, Serialize};

pub const APU_REG_START: usize = 0x04000060;
pub const APU_REG_END: usize = 0x040000A9;

#[derive(Deserialize, Serialize)]
pub struct Apu {
    #[serde(with = "serde_arrays")]
    pub regs: [u8; APU_REG_END - APU_REG_START],

    pub buffer: Vec<f32>,
}
