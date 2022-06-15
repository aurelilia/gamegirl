use serde::{Deserialize, Serialize};

pub const DMA_WIDTH: usize = 10;

#[derive(Deserialize, Serialize)]
pub struct Dma {
    pub regs: [u8; DMA_WIDTH],
}
