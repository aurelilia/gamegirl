use crate::{interface::Bus, Cpu};

impl<S: Bus> Cpu<S> {
    pub fn setup_cpu_state(&mut self) {
        log::debug!("hi from jit block running at {}", self.state.pc());
        self.state.pipeline_valid = true;
    }

    pub fn tick_bus(&mut self, by: u16) {
        self.bus.tick(by as u64);
    }

    pub extern "C" fn set_nz_(&mut self, value: u32) {
        self.set_nz(true, value);
    }

    pub extern "C" fn set_nzc_(&mut self, value: u32, carry: bool) {
        self.set_nzc(true, value, carry);
    }

    pub extern "C" fn set_nzcv_(&mut self, value: u32, carry: bool, overflow: bool) {
        self.set_nzcv(true, value, carry, overflow);
    }
}
