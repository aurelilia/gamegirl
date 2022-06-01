use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub struct Debugger {
    pub serial_output: Mutex<String>,

    pub breakpoints: Mutex<Vec<Breakpoint>>,
    pub breakpoints_enabled: AtomicBool,
    pub breakpoint_hit: AtomicBool,
}

impl Debugger {
    pub fn write_occurred(&self, addr: u16) {
        let hit = self
            .breakpoints
            .lock()
            .unwrap()
            .iter()
            .any(|bp| bp.addr == Some(addr) && bp.write);
        if hit {
            self.breakpoint_hit.store(true, Ordering::Relaxed);
        }
    }

    pub fn should_execute(&self, pc: u16) -> bool {
        let hit = self
            .breakpoints
            .lock()
            .unwrap()
            .iter()
            .any(|bp| bp.addr == Some(pc) && bp.pc);
        if hit {
            self.breakpoint_hit.store(true, Ordering::Relaxed);
        }
        !hit
    }
}

#[derive(Debug, Default)]
pub struct Breakpoint {
    pub addr: Option<u16>,
    pub addr_text: String,
    pub pc: bool,
    pub write: bool,
}
