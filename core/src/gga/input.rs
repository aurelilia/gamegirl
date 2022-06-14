use serde::{Deserialize, Serialize};

pub const INPUT_START: usize = 0x04000130;
pub const INPUT_END: usize = 0x04000133;

#[derive(Deserialize, Serialize)]
pub struct Input {
    pub regs: [u8; 4],
}
