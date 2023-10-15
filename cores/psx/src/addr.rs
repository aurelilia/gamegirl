pub const MMIOBASE: u32 = 0x1F80_1000;

// DMA
pub const DMABASE: u32 = 0x080;
pub const DMAADDR: u32 = 0x0;
pub const DMABLOCKCTRL: u32 = 0x4;
pub const DMACHCTRL: u32 = 0x08;
pub const DMACTRL: u32 = 0x0F0;
pub const DMAINT: u32 = 0x0F4;

pub const PORT_GPU: u32 = 0x2;
pub const PORT_OTC: u32 = 0x6;

// GPU
pub const GPUREAD: u32 = 0x810;
pub const GPUSTAT: u32 = 0x814;
pub const GP0: u32 = 0x810;
pub const GP1: u32 = 0x814;
