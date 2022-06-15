use serde::{Deserialize, Serialize};

pub const TIMER_WIDTH: usize = 4;

#[derive(Deserialize, Serialize)]
pub struct Timer {
    pub regs: [u8; TIMER_WIDTH],
}
