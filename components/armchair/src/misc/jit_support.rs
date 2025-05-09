use crate::{interface::Bus, Cpu};

impl<S: Bus> Cpu<S> {
    pub fn print_thing(&mut self) {
        log::error!("hi from jit block running at {}", self.state.pc())
    }
}
