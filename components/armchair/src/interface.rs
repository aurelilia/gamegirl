// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::{common::debugger::Debugger, numutil::NumExt, Time};

use crate::{
    arm::{self, ArmInstructionSet},
    memory::{Access, Address},
    state::CpuState,
    thumb::{self, ThumbInstructionSet},
    Cpu, Exception,
};

/// Configuration that specifies how the system should behave on the bus.
pub struct BusCpuConfig {
    /// Base address for exception vectors
    pub exception_vector_base_address: Address,
}

/// Trait for a system that contains this CPU.
pub trait Bus: Sized + 'static {
    /// CPU version to emulate for this bus.
    type Version: CpuVersion<Self>;

    /// System configuration.
    const CONFIG: BusCpuConfig;

    /// Increment the system clock by the given amount of CPU ticks.
    fn tick(&mut self, cycles: Time);
    /// Handle pending events on the bus.
    fn handle_events(&mut self, cpu: &mut CpuState);
    /// Get the system debugger.
    fn debugger(&mut self) -> &mut Debugger;

    /// Callback to perform any system-specific behavior on an exception.
    fn exception_happened(&mut self, cpu: &mut CpuState, kind: Exception);
    /// Callback to perform any system-specific behavior on a pipeline stall.
    fn pipeline_stalled(&mut self, cpu: &mut CpuState);

    /// Get the value at the given memory address.
    fn get<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address) -> T;
    /// Set the value at the given memory address.
    fn set<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address, value: T);
    /// Get the access time in S/N cycles for the given memory address.
    /// The type is mut here due to things like the prefetch buffer,
    /// which changes state when accessed.
    fn wait_time<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address, access: Access) -> u16;

    /// Get the value at the given memory address and add to the system clock.
    fn read<T: RwType>(
        &mut self,
        cpu: &mut CpuState,
        addr: Address,
        access: Access,
    ) -> T::ReadOutput {
        let time = self.wait_time::<T>(cpu, addr, access);
        self.tick(time as u64);

        let value = self.get::<T>(cpu, addr).u32();
        T::ReadOutput::from_u32(if !Self::Version::IS_V5 && T::WIDTH == 2 {
            // Special handling for halfwords on ARMv4
            if addr.0.is_bit(0) {
                // Unaligned
                value.u32().rotate_right(8)
            } else {
                value.u32()
            }
        } else {
            value
        })
    }
    /// Set the value at the given memory address and add to the system clock.
    fn write<T: RwType>(&mut self, cpu: &mut CpuState, addr: Address, value: T, access: Access) {
        let time = self.wait_time::<T>(cpu, addr, access);
        self.tick(time as u64);
        self.debugger().write_occurred(addr.0);
        self.set(cpu, addr, value);
    }

    /// Callback for getting CP15 register.
    /// This CPU implementation relies on the system to provide the CP15
    /// implementation. It is only used when `IS_V5 == true`
    fn get_cp15(&self, _cm: u32, _cp: u32, _cn: u32) -> u32 {
        panic!("CP15 unsupported!")
    }
    /// Callback for setting CP15 register.
    /// This CPU implementation relies on the system to provide the CP15
    /// implementation. It is only used when `IS_V5 == true`
    fn set_cp15(&mut self, _cm: u32, _cp: u32, _cn: u32, _rd: u32) {
        panic!("CP15 unsupported!");
    }
}

/// Trait for a CPU version to implement.
pub trait CpuVersion<S: Bus> {
    /// If this version exhibits V5 behavior.
    const IS_V5: bool;
    /// Thumb instruction set.
    const THUMB: ThumbInstructionSet<S>;
    /// Arm instruction set.
    const ARM: ArmInstructionSet<S>;
}

pub struct InstructionSet<S: Bus, I, const LUT_SIZE: usize> {
    pub(crate) interpreter_lut: [fn(&mut Cpu<S>, I); LUT_SIZE],
    pub(crate) cache_handler_lookup: fn(I) -> fn(&mut Cpu<S>, I),
}

pub struct Arm7Dtmi;

impl<S: Bus> CpuVersion<S> for Arm7Dtmi {
    const IS_V5: bool = false;
    const THUMB: ThumbInstructionSet<S> = thumb::instruction_set();
    const ARM: ArmInstructionSet<S> = arm::instruction_set();
}

pub struct Arm946Es;

impl<S: Bus> CpuVersion<S> for Arm946Es {
    const IS_V5: bool = true;
    const THUMB: ThumbInstructionSet<S> = thumb::instruction_set();
    const ARM: ArmInstructionSet<S> = arm::instruction_set();
}

/// Trait for a type that the CPU can read/write memory with.
/// On this ARM CPU, it is u8, u16, u32.
pub trait RwType: NumExt + 'static {
    type ReadOutput: RwType;
}

impl RwType for u8 {
    type ReadOutput = Self;
}

impl RwType for u16 {
    /// u16 outputs u32: On unaligned reads, the CPU
    /// shifts the result, therefore making it 32bit.
    type ReadOutput = u32;
}

impl RwType for u32 {
    type ReadOutput = Self;
}
