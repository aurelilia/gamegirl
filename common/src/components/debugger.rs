// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

/// Debugger info that is required to be known by the system.
/// Is generic over GGC/GGA; generic type Ptr is pointer size
/// on the current system (u16/u32)
#[derive(Clone, Default)]
pub struct Debugger<Ptr: PartialEq + Clone + Copy> {
    /// Contains the serial output that was written to IO register SB.
    /// Currently only on GG.
    pub serial_output: String,
    /// If the system is running. If false, any calls to functions advancing
    /// the system based on outside sources (time, sound) will do nothing.
    pub running: bool,
    /// A list of breakpoints the system should stop on.
    pub breakpoints: Vec<Breakpoint<Ptr>>,
    /// The hit breakpoint's index.
    pub breakpoint_hit: Option<usize>,
    /// If instructions should be traced and printed to a file, this contains
    /// the instructions to be printed / file contents.
    pub traced_instructions: Option<String>,
}

impl<Ptr: PartialEq + Clone + Copy> Debugger<Ptr> {
    /// Called before a memory write is executed, which might trigger a BP.
    /// Returns if emulation should continue.
    pub fn write_occurred(&mut self, addr: Ptr) {
        if !self.breakpoints.is_empty() {
            let bp = self
                .breakpoints
                .iter()
                .position(|bp| bp.value == Some(addr) && bp.write);
            self.breakpoint_hit = bp;
            self.running &= bp.is_none();
        }
    }

    /// Called before an instruction is executed, which might trigger a BP.
    /// If it does, function returns false and inst should not be executed.
    pub fn should_execute(&mut self, pc: Ptr) -> bool {
        if self.breakpoints.is_empty() {
            return true;
        }

        if self.breakpoint_hit.take().is_some() {
            // We hit a breakpoint already. Continue
            return true;
        }

        let bp = self
            .breakpoints
            .iter()
            .position(|bp| bp.value == Some(pc) && bp.pc);
        self.breakpoint_hit = bp;
        self.running &= bp.is_none();
        bp.is_none()
    }

    #[inline]
    pub fn tracing(&self) -> bool {
        self.traced_instructions.is_some()
    }

    /// Add another instruction to trace.
    pub fn add_traced_instruction(&mut self, writer: impl FnOnce() -> String) {
        if let Some(instr) = self.traced_instructions.as_mut() {
            let text = writer();
            instr.push('\n');
            instr.push_str(&text);
        }
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
