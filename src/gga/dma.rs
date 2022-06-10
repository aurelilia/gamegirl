pub const DMA_START: usize = 0x040000B0;
pub const DMA_END: usize = 0x40000DF;
pub const DMA_WIDTH: usize = 10;

#[derive(Debug, Clone)]
pub struct DMA {
    pub regs: [u8; DMA_WIDTH],
}
