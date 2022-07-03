/// Debugger info that is required to be known by the system.
/// Is generic over GGC/GGA; generic type Ptr is pointer size
/// on the current system (u16/u32)
#[derive(Clone, Default)]
pub struct Debugger<Ptr: PartialEq + Clone + Copy> {
    /// Contains the serial output that was written to IO register SB.
    /// Currently only on GG.
    pub serial_output: String,
    /// A list of breakpoints the system should stop on.
    pub breakpoints: Vec<Breakpoint<Ptr>>,
    /// If a breakpoint was hit.
    pub is_breakpoint_hit: bool,
    /// The hit breakpoint.
    pub breakpoint_hit: Option<Breakpoint<Ptr>>,
}

impl<Ptr: PartialEq + Clone + Copy> Debugger<Ptr> {
    /// Called before a memory write is executed, which might trigger a BP.
    pub fn write_occurred(&mut self, addr: Ptr) {
        if self.breakpoints.is_empty() {
            return;
        }

        let bp = self
            .breakpoints
            .iter()
            .find(|bp| bp.value == Some(addr) && bp.write);
        if let Some(bp) = bp {
            self.breakpoint_hit = Some(bp.clone());
            self.is_breakpoint_hit = true;
        }
    }

    /// Called before an instruction is executed, which might trigger a BP.
    /// If it does, function returns false and inst should not be executed.
    pub fn should_execute(&mut self, pc: Ptr) -> bool {
        if self.breakpoints.is_empty() {
            return true;
        }

        let bp = self
            .breakpoints
            .iter()
            .find(|bp| bp.value == Some(pc) && bp.pc);
        if let Some(bp) = bp {
            self.breakpoint_hit = Some(bp.clone());
            self.is_breakpoint_hit = true;
        }
        !self.is_breakpoint_hit
    }
}

/// A breakpoint.
#[derive(Clone, Debug, Default)]
pub struct Breakpoint<Ptr> {
    /// Address/value that this breakpoint is at.
    pub value: Option<Ptr>,
    /// String representation of the address/value; used by egui as a text
    /// buffer. TODO: kinda unclean to have GUI state here...
    pub value_text: String,
    /// If this breakpoint triggers on the PC.
    pub pc: bool,
    /// If this breakpoint triggers on a write.
    pub write: bool,
}
