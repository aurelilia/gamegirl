pub const APUREG_START: usize = 0x04000060;
pub const APUREG_END: usize = 0x040000A9;

#[derive(Debug, Clone)]
pub struct APU {
    pub regs: [u8; APUREG_END - APUREG_START],
}
