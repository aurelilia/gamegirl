use super::{OptEntry, OptimizationData};
use crate::{
    interface::Bus,
    memory::{Address, RelativeOffset},
    state::CpuState,
    Cpu,
};

/// Data for waitloop detection
/// This allows detecting loops that either loop without R/W,
/// or loop on reading a single memory address.
#[derive(Debug, Clone, Copy, Default)]
pub enum WaitloopData {
    /// Currently not detecting anything
    #[default]
    None,
    /// We saw a suspicous jump to an address close before PC
    SuspicousJump { br_address: Address },
    /// That jump repeated, we're in a loop. Find R/W
    /// (If more than 1 read or writes are found, discard.)
    FindReads {
        br_address: Address,
        read: Option<ReadValue>,
    },
    /// This is confirmed a loop we can waitloop on. Make sure that the register
    /// values are the same as when we first saw the jump, to make
    /// sure the game isn't doing something else.
    CheckRegisters {
        regs: [u32; 15],
        read: Option<ReadValue>,
    },
    /// We're in a loop reading a single memory value
    InLoopMem { memory: ReadValue },
}

impl WaitloopData {
    pub fn on_read(&mut self, addr: Address, value: u32, table: &mut OptimizationData) {
        *self = match *self {
            WaitloopData::FindReads {
                br_address,
                read: None,
            } => WaitloopData::FindReads {
                br_address,
                read: Some(ReadValue { addr, value }),
            },

            WaitloopData::FindReads {
                br_address,
                read: Some(_),
            } => {
                table.get_or_create_entry(br_address).unwrap().waitloop = WaitloopPoint::Ignore;
                WaitloopData::None
            }

            WaitloopData::SuspicousJump { .. }
            | WaitloopData::CheckRegisters { .. }
            | WaitloopData::InLoopMem { .. } => return,

            _ => WaitloopData::None,
        };
    }

    pub fn on_write(&mut self, table: &mut OptimizationData) {
        match *self {
            WaitloopData::SuspicousJump { br_address }
            | WaitloopData::FindReads { br_address, .. } => {
                table.get_or_create_entry(br_address).unwrap().waitloop = WaitloopPoint::Ignore;
            }
            _ => (),
        }
        *self = WaitloopData::None;
    }

    /// To be called before a relative jump.
    /// Returns if CPU is still running.
    pub fn on_jump(
        &mut self,
        regs: &CpuState,
        dest: RelativeOffset,
        table: &mut OptimizationData,
    ) -> bool {
        if !(-16..0).contains(&dest.0) {
            return true;
        }

        *self = match self {
            WaitloopData::None => {
                match table.get_or_create_entry(regs.pc()) {
                    Some(OptEntry {
                        waitloop: WaitloopPoint::Ignore,
                        ..
                    }) => return true,
                    Some(OptEntry {
                        waitloop: WaitloopPoint::IrqLoop,
                        ..
                    }) => return false,
                    Some(OptEntry {
                        waitloop: WaitloopPoint::MemoryLoop { mem },
                        ..
                    }) => {
                        *self = WaitloopData::InLoopMem { memory: *mem };
                        return false;
                    }

                    Some(OptEntry {
                        waitloop: WaitloopPoint::Unanalyzed,
                        ..
                    }) => (),

                    None => return true,
                };

                WaitloopData::SuspicousJump {
                    br_address: regs.pc(),
                }
            }

            WaitloopData::SuspicousJump { br_address: prev } if *prev == regs.pc() => {
                WaitloopData::FindReads {
                    br_address: *prev,
                    read: None,
                }
            }
            WaitloopData::SuspicousJump { .. } => {
                table.get_or_create_entry(regs.pc()).unwrap().waitloop = WaitloopPoint::Ignore;
                WaitloopData::None
            }

            WaitloopData::FindReads { read, .. } => WaitloopData::CheckRegisters {
                regs: (regs.registers[0..15]).try_into().unwrap(),
                read: *read,
            },

            WaitloopData::CheckRegisters {
                regs: prev,
                read: Some(memory),
            } if *prev == regs.registers[0..15] => {
                // Waitlooping on memory
                table.get_or_create_entry(regs.pc()).unwrap().waitloop =
                    WaitloopPoint::MemoryLoop { mem: *memory };
                *self = WaitloopData::InLoopMem { memory: *memory };
                return false;
            }
            WaitloopData::CheckRegisters {
                regs: prev,
                read: None,
            } if *prev == regs.registers[0..15] => {
                // Waitlooping on INTR
                // Regular interrupt checking code will initiate resume later
                table.get_or_create_entry(regs.pc()).unwrap().waitloop = WaitloopPoint::IrqLoop;
                return false;
            }
            // Registers were different, the game is doing something weird.
            // Do not trigger waitloop detection.
            WaitloopData::CheckRegisters { .. } => {
                table.get_or_create_entry(regs.pc()).unwrap().waitloop = WaitloopPoint::Ignore;
                WaitloopData::None
            }

            WaitloopData::InLoopMem { .. } => WaitloopData::None,
        };
        true
    }
}

impl<S: Bus> Cpu<S> {
    pub fn check_unsuspend(&mut self) {
        let intr_pending = (self.state.intr.ie & self.state.intr.if_) != 0;
        self.state.is_halted = !(intr_pending || self.in_waitloop_ready_for_resume());
    }

    fn in_waitloop_ready_for_resume(&mut self) -> bool {
        match self.opt.waitloop {
            WaitloopData::InLoopMem { memory } => {
                let value = self.bus.get::<u32>(&mut self.state, memory.addr);
                value != memory.value
            }
            _ => false,
        }
    }
}

#[derive(Copy, Clone)]
pub enum WaitloopPoint {
    Unanalyzed,
    Ignore,
    IrqLoop,
    MemoryLoop { mem: ReadValue },
}

/// Value the program is reading in a loop.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadValue {
    /// Address being read
    addr: Address,
    /// Value that is causing us to stay in the loop
    value: u32,
}
