use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Debugger info that is required to be known by the system.
#[derive(Default)]
pub struct Debugger {
    /// Contains the serial output that was written to IO register SB.
    pub serial_output: Mutex<String>,

    /// A list of breakpoints the system should stop on.
    pub breakpoints: Mutex<Vec<Breakpoint>>,
    /// If breakpoints are enabled.
    pub breakpoints_enabled: AtomicBool,
    /// If a breakpoint was hit.
    pub breakpoint_hit: AtomicBool,
}

impl Debugger {
    /// Called before a memory write is executed, which might trigger a BP.
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

    /// Called before an instruction is executed, which might trigger a BP.
    /// If it does, function returns false and inst should not be executed.
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

/// A breakpoint.
#[derive(Debug, Default)]
pub struct Breakpoint {
    /// Address that this breakpoint is at.
    pub addr: Option<u16>,
    /// String representation of the address; used by egui as a text buffer.
    /// TODO: kinda unclean to have GUI state here...
    pub addr_text: String,
    /// If this breakpoint triggers on the PC.
    pub pc: bool,
    /// If this breakpoint triggers on a write.
    pub write: bool,
}
