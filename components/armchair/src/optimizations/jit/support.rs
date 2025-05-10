use crate::{interface::Bus, Cpu};

impl<S: Bus> Cpu<S> {
    pub fn setup_cpu_state(&mut self) {
        log::debug!("hi from jit block running at {}", self.state.pc());
        self.state.pipeline_valid = true;
    }

    pub fn tick_bus(&mut self, by: u16) {
        self.bus.tick(by as u64);
    }
}
