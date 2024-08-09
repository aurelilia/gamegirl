// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use crate::{
    interface::{ArmSystem, SysWrapper},
    Cpu,
};

impl<S: ArmSystem> Cpu<S> {
    /// Check if the CPU should be unsuspended
    pub fn check_unsuspend(gg: &mut S) {
        let intr_pending = (gg.cpur().ie & gg.cpur().if_) != 0;
        gg.cpu().is_halted = !intr_pending && !Self::check_waitloop_resume(gg);
    }

    /// Immediately halt the CPU until an IRQ is pending
    pub fn halt_on_irq(&mut self) {
        self.is_halted = true;
        self.waitloop = WaitloopData::InLoopIrq;
    }

    /// Check if the value we were waitlooping on changed, if applicable
    fn check_waitloop_resume(gg: &mut S) -> bool {
        let gg = SysWrapper::new(gg);
        match gg.cpu().waitloop {
            WaitloopData::InLoopIrq => false,
            WaitloopData::InLoopMem { memory } => {
                let value = gg.get::<u32>(memory.addr);
                value & memory.mask != memory.value
            }
            _ => true,
        }
    }
}

/// Value the program is reading in a loop.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadValue {
    /// Address being read
    addr: u32,
    /// Value that is causing us to stay in the loop
    value: u32,
    /// Mask to apply to the value (e.g. for 16/8-bit reads)
    mask: u32,
}

/// Data for waitloop detection
/// This allows detecting loops that either loop without R/W,
/// or loop on reading a single memory address.
#[derive(Debug, Clone, Copy, Default)]
pub enum WaitloopData {
    /// Currently not detecting anything
    #[default]
    None,
    /// We saw a suspicous jump to an address close before PC
    SuspicousJump { br_address: u32 },
    /// That jump repeated, we're in a loop. Find R/W
    /// (If more than 1 read or writes are found, discard.)
    FindReads {
        br_address: u32,
        read: Option<ReadValue>,
    },
    /// This is confirmed a loop we can waitloop on. Make sure that the register
    /// values are the same as when we first saw the jump, to make
    /// sure the game isn't doing something else.
    CheckHash {
        regs_hash: u64,
        read: Option<ReadValue>,
    },
    /// We're in a loop reading a single memory value
    InLoopMem { memory: ReadValue },
    /// We're in a loop waiting for IRQ
    /// This variant is also used when waiting via some low-power state,
    /// like HALTCNT on the GBA
    InLoopIrq,
}

impl WaitloopData {
    /// Returns if CPU is still running.
    pub fn on_jump(&mut self, regs: &[u32; 16], br_address: u32, dest: i32) -> bool {
        if !(-16..0).contains(&dest) {
            return true;
        }
        *self = match self {
            WaitloopData::None => WaitloopData::SuspicousJump { br_address },

            WaitloopData::SuspicousJump { br_address: prev } if *prev == br_address => {
                WaitloopData::FindReads {
                    br_address,
                    read: None,
                }
            }
            WaitloopData::SuspicousJump { .. } => WaitloopData::None,

            WaitloopData::FindReads { read, .. } => WaitloopData::CheckHash {
                regs_hash: hash(regs),
                read: *read,
            },

            WaitloopData::CheckHash {
                regs_hash,
                read: Some(memory),
            } if *regs_hash == hash(regs) => {
                // Waitlooping on memory
                *self = WaitloopData::InLoopMem { memory: *memory };
                return false;
            }
            WaitloopData::CheckHash {
                regs_hash,
                read: None,
            } if *regs_hash == hash(regs) => {
                // Waitlooping on INTR
                *self = WaitloopData::InLoopIrq;
                return false;
            }
            // Registers were different, the game is doing something weird.
            // Do not trigger waitloop detection.
            WaitloopData::CheckHash { .. } => WaitloopData::None,

            WaitloopData::InLoopMem { .. } | WaitloopData::InLoopIrq => WaitloopData::None,
        };
        true
    }

    pub fn on_read(&mut self, addr: u32, value: u32, mask: u32) {
        *self = match *self {
            WaitloopData::FindReads {
                br_address,
                read: None,
            } => WaitloopData::FindReads {
                br_address,
                read: Some(ReadValue { addr, value, mask }),
            },
            WaitloopData::SuspicousJump { .. }
            | WaitloopData::CheckHash { .. }
            | WaitloopData::InLoopMem { .. }
            | WaitloopData::InLoopIrq => return,

            _ => WaitloopData::None,
        };
    }

    pub fn on_write(&mut self) {
        *self = WaitloopData::None;
    }
}

fn hash(regs: &[u32; 16]) -> u64 {
    // FNV-1
    let init = 0xcbf29ce484222325;
    let prime = 0x100000001b3;
    regs.iter()
        .flat_map(|u| u.to_le_bytes())
        .fold(init, |hash, byte| hash.wrapping_mul(prime) ^ byte as u64)
}
