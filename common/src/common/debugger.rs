// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{cmp::Ordering, fmt::Debug, sync::Mutex, time::Instant};

use crate::{numutil::NumExt, Pointer};

/// Debugger info that is required to be known by the system.
/// Is generic over cores.
#[derive(Default)]
pub struct Debugger {
    /// Contains the serial output of the console. Exact meaning depends on
    /// the platform.
    pub serial_output: String,
    /// If instructions should be traced and printed to a file, this contains
    /// the instructions to be printed / file contents.
    pub traced_instructions: Option<String>,

    /// If the system is running. If false, any calls to functions advancing
    /// the system based on outside sources (time, sound) will do nothing.
    pub running: bool,

    /// A list of breakpoints the system should stop on.
    pub breakpoints: Vec<Breakpoint>,
    /// The hit breakpoint's index.
    pub breakpoint_hit: Option<usize>,

    /// The diagnostic level that is currently enabled.
    /// Any diagnostic events with a severity lower than this will not be
    /// logged and discarded.
    pub diagnostic_level: Severity,
    /// Diagnostic events that have occurred.
    pub diagnostic_events: Mutex<Vec<DiagnosticEvent>>,
}

impl Debugger {
    /// Called before a memory write is executed, which might trigger a BP.
    /// Returns if emulation should continue.
    pub fn write_occurred(&mut self, addr: Pointer) {
        if !self.breakpoints.is_empty() {
            let bp = self
                .breakpoints
                .iter()
                .position(|bp| bp.value == Some(addr) && bp.write);
            self.breakpoint_hit = bp;
            self.running &= bp.is_none();
            if !self.running {
                self.add_traced_instruction(|| format!("Write to Breakpoint at {:#X}", addr));
            }
        }
    }

    /// Called before an instruction is executed, which might trigger a BP.
    /// If it does, function returns false and inst should not be executed.
    pub fn should_execute(&mut self, pc: Pointer) -> bool {
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

    /// Log a diagnostic event that occured, if the corresponding level
    /// is enabled.
    pub fn log(&self, evt_type: &str, event: String, severity: Severity) {
        if severity >= self.diagnostic_level {
            self.diagnostic_events
                .lock()
                .unwrap()
                .push(DiagnosticEvent {
                    evt_type: evt_type.to_string(),
                    event,
                    severity,
                    time: Instant::now(),
                    state: None,
                });
        }
    }
}

/// A breakpoint.
#[derive(Clone, Debug, Default)]
pub struct Breakpoint {
    /// Address/value that this breakpoint is at.
    pub value: Option<Pointer>,
    /// String representation of the address/value; used by egui as a text
    /// buffer. TODO: kinda unclean to have GUI state here...
    pub value_text: String,
    /// If this breakpoint triggers on the PC.
    pub pc: bool,
    /// If this breakpoint triggers on a write.
    pub write: bool,
}

/// A diagnostic event that might be interesting during debugging.
#[derive(Debug)]
pub struct DiagnosticEvent {
    /// The type of the event.
    /// This is used to for breakpointing on specific events.
    pub evt_type: String,
    /// The display message of what occurred.
    pub event: String,
    /// The severity of the event.
    pub severity: Severity,
    /// The time the event occurred .
    pub time: Instant,
    /// Save state, if enabled, to be used to aid debugging.
    pub state: Option<Vec<u8>>,
}

/// The severity of a diagnostic event.
/// Event severity is decided by the system and can be used to filter.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub enum Severity {
    Debug = 0,
    Info = 10,
    Warning = 100,
    Error = 1000,
    #[default]
    None = 10000,
}

impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((*self as u32).cmp(&(*other as u32)))
    }
}

/// Width of a value to be read/written from memory.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    #[default]
    Byte,
    Halfword,
    Word,
}

impl Width {
    /// Returns the size of the values represented by this width.
    pub fn size(&self) -> usize {
        match self {
            Width::Byte => 1,
            Width::Halfword => 2,
            Width::Word => 4,
        }
    }

    /// Returns the mask for the width, to be applied when working with values
    /// of this width.
    pub fn mask(&self) -> u32 {
        match self {
            Width::Byte => 0xFF,
            Width::Halfword => 0xFFFF,
            Width::Word => 0xFFFFFFFF,
        }
    }
}

/// Search an array for a value.
/// Adds found values to the matches vector with specified offset.
pub fn search_array(
    matches: &mut Vec<u32>,
    arr: &[u8],
    offset: u32,
    value: u32,
    width: Width,
    kind: Ordering,
) {
    for (i, chunk) in arr.chunks_exact(width.size()).enumerate() {
        let chunk = match width {
            Width::Byte => chunk[0].u32(),
            Width::Halfword => u16::from_le_bytes([chunk[0], chunk[1]]).u32(),
            Width::Word => u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        };
        if chunk.cmp(&value) == kind {
            matches.push(offset + i.u32() * width.size().u32());
        }
    }
}
