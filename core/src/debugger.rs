/// Debugger info that is required to be known by the system.
/// Is generic over GGC/GGA; generic type Ptr is pointer size
/// on the current system (u16/u32)
#[derive(Clone, Default)]
pub struct Debugger<Ptr: PartialEq + Clone + Copy> {
    /// Contains the serial output that was written to IO register SB.
    pub serial_output: String,

    /// A list of breakpoints the system should stop on.
    pub breakpoints: Vec<Breakpoint<Ptr>>,
    /// If breakpoints are enabled.
    pub breakpoints_enabled: bool,
    /// If a breakpoint was hit.
    pub breakpoint_hit: bool,
}

impl<Ptr: PartialEq + Clone + Copy> Debugger<Ptr> {
    /// Called before a memory write is executed, which might trigger a BP.
    pub fn write_occurred(&mut self, addr: Ptr) {
        if !self.breakpoints_enabled {
            return;
        }
        self.breakpoint_hit |= self
            .breakpoints
            .iter()
            .any(|bp| bp.addr == Some(addr) && bp.write);
    }

    /// Called before an instruction is executed, which might trigger a BP.
    /// If it does, function returns false and inst should not be executed.
    pub fn should_execute(&mut self, pc: Ptr) -> bool {
        if !self.breakpoints_enabled {
            return true;
        }
        !self
            .breakpoints
            .iter()
            .any(|bp| bp.addr == Some(pc) && bp.pc)
    }
}

/// A breakpoint.
#[derive(Clone, Debug, Default)]
pub struct Breakpoint<Ptr> {
    /// Address that this breakpoint is at.
    pub addr: Option<Ptr>,
    /// String representation of the address; used by egui as a text buffer.
    /// TODO: kinda unclean to have GUI state here...
    pub addr_text: String,
    /// If this breakpoint triggers on the PC.
    pub pc: bool,
    /// If this breakpoint triggers on a write.
    pub write: bool,
}
