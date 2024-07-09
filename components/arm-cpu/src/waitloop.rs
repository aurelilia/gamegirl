use crate::{
    interface::{ArmSystem, SysWrapper},
    Cpu,
};

impl<S: ArmSystem> Cpu<S> {
    /// Check if the CPU should be unsuspended
    pub fn check_unsuspend(gg: &mut S) {
        let intr_pending = (gg.cpur().ie & gg.cpur().if_) != 0;
        let waitloop_complete = Self::check_waitloop_resume(gg);
        gg.cpu().is_halted = !intr_pending && !waitloop_complete;
    }

    /// Check if the value we were waitlooping on changed, if applicable
    fn check_waitloop_resume(gg: &mut S) -> bool {
        let gg = SysWrapper::new(gg);
        let WaitloopData::InLoopMem { memory } = gg.cpu().waitloop else {
            return true;
        };

        let value = gg.get::<u32>(memory.addr);
        value & memory.mask != memory.value
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
    /// We're in a loop reading a single memory value
    InLoopMem { memory: ReadValue },
}

impl WaitloopData {
    /// Returns if CPU is still running.
    pub fn on_jump(&mut self, br_address: u32, dest: i32) -> bool {
        if !(-0xF..0x0).contains(&dest) {
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

            WaitloopData::FindReads { read: None, .. } => {
                // Waitlooping on INTR
                log::warn!("Waitlooping on INTR");
                return false;
            }
            WaitloopData::FindReads {
                read: Some(memory), ..
            } => {
                *self = WaitloopData::InLoopMem { memory: *memory };
                return false; // TODO
            }

            WaitloopData::InLoopMem { .. } => WaitloopData::None,
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
            WaitloopData::SuspicousJump { .. } | WaitloopData::InLoopMem { .. } => return,

            _ => WaitloopData::None,
        };
    }

    pub fn on_write(&mut self) {
        *self = WaitloopData::None;
    }
}
