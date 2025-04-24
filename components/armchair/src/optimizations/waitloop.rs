use crate::{
    memory::{Address, RelativeOffset},
    registers::Registers,
};

#[derive(Default)]
pub struct WaitloopData {}

impl WaitloopData {
    pub fn on_read(&mut self, addr: Address, value: u32, mask: u32) {}
    pub fn on_write(&mut self) {}

    /// To be called before a relative jump.
    /// Returns if CPU is still running.
    pub fn on_jump(&mut self, regs: &Registers, dest: RelativeOffset) -> bool {
        true
    }
}
