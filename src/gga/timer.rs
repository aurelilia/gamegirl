pub const TIMER_START: usize = 0x04000100;
pub const TIMER_END: usize = 0x400010F;
pub const TIMER_WIDTH: usize = 4;

pub struct Timer {
    pub regs: [u8; TIMER_WIDTH],
}
