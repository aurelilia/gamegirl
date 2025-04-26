use crate::{
    interface::Bus,
    memory::{Address, RelativeOffset},
    state::CpuState,
    Cpu,
};

#[derive(Default)]
pub struct WaitloopData {}

impl WaitloopData {
    pub fn on_read(&mut self, addr: Address, value: u32, mask: u32) {}
    pub fn on_write(&mut self) {}

    /// To be called before a relative jump.
    /// Returns if CPU is still running.
    pub fn on_jump(&mut self, regs: &CpuState, dest: RelativeOffset) -> bool {
        true
    }
}

impl<S: Bus> Cpu<S> {
    pub fn check_unsuspend(&mut self) {}
}
