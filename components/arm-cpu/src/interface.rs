// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::{Deref, DerefMut, IndexMut};

use common::numutil::NumExt;

use super::Exception;
use crate::{inst_arm::ArmLut, inst_thumb::ThumbLut, Access, Cpu};

/// Trait for a system that contains this CPU.
pub trait ArmSystem: IndexMut<u32, Output = u16> + Sized + 'static {
    /// Is this an ARMv5 system? ARMv4 otherwise.
    const IS_V5: bool;
    /// LUT for ARM instructions.
    const ARM_LUT: ArmLut<Self> = SysWrapper::<Self>::make_arm_lut();
    /// LUT for THUMB instructions.
    const THUMB_LUT: ThumbLut<Self> = SysWrapper::<Self>::make_thumb_lut();
    /// Address of the lowest byte of IF; used when raising interrupts
    const IF_ADDR: u32;

    /// Get the CPU.
    fn cpur(&self) -> &Cpu<Self>;
    /// Get the CPU.
    fn cpu(&mut self) -> &mut Cpu<Self>;

    /// Advance the system besides the CPU.
    fn advance_clock(&mut self);
    /// Add S or N cycles to the system clock.
    fn add_sn_cycles(&mut self, cycles: u16);
    /// Add I cycles to the system clock.
    fn add_i_cycles(&mut self, cycles: u16);

    /// Should return if there is an IRQ pending.
    /// If true, CPU will raise an IRQ exception.
    fn is_irq_pending(&self) -> bool;
    /// Callback to perform any system-specific behavior on an exception.
    fn exception_happened(&mut self, kind: Exception);
    /// Callback to perform any system-specific behavior on a pipeline stall.
    fn pipeline_stalled(&mut self);

    /// Get the value at the given memory address.
    fn get<T: RwType>(&mut self, addr: u32) -> T;
    /// Set the value at the given memory address.
    fn set<T: RwType>(&mut self, addr: u32, value: T);
    /// Get the access time in S/N cycles for the given memory address.
    /// The type is mut here due to things like the prefetch buffer,
    /// which changes state when accessed.
    fn wait_time<T: RwType>(&mut self, addr: u32, access: Access) -> u16;

    /// Get the value at the given memory address and add to the system clock.
    fn read<T: RwType>(&mut self, addr: u32, access: Access) -> T::ReadOutput {
        let time = self.wait_time::<T>(addr, access);
        self.add_sn_cycles(time);
        let value = self.get::<T>(addr).u32();
        T::ReadOutput::from_u32(if !Self::IS_V5 && T::WIDTH == 2 {
            // Special handling for halfwords on ARMv4
            if addr.is_bit(0) {
                // Unaligned
                Cpu::<Self>::ror_s0(value.u32(), 8)
            } else {
                value.u32()
            }
        } else {
            value
        })
    }
    /// Set the value at the given memory address and add to the system clock.
    fn write<T: RwType>(&mut self, addr: u32, value: T, access: Access) {
        let time = self.wait_time::<T>(addr, access);
        self.add_sn_cycles(time);
        self.set(addr, value);
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
    fn set_cp15(&self, _cm: u32, _cp: u32, _cn: u32, _rd: u32) {
        panic!("CP15 unsupported!");
    }

    /// Check with the system debugger if the current instruction is to be
    /// executed. If this returns false, the CPU aborts execution.
    fn check_debugger(&mut self) -> bool;
    /// Check if the current instruction can be used to start creating an
    /// instruction cache block.
    fn can_cache_at(addr: u32) -> bool;
}

/// Wrapper for the system that adds a few utility functions.
/// TODO: Does this really have a good reason to exist? Might
/// be better to just move these functions somewhere else
/// and not bother with a wrapper.
#[repr(transparent)]
pub struct SysWrapper<S: ArmSystem> {
    pub inner: *mut S,
}

impl<S: ArmSystem> SysWrapper<S> {
    /// Read a half-word from the bus (LE).
    /// If address is unaligned, do LDRSH behavior.
    pub fn read_hword_ldrsh(&mut self, addr: u32, kind: Access) -> u32 {
        let time = self.wait_time::<u16>(addr, kind);
        self.add_sn_cycles(time);
        let val = self.get::<u16>(addr).u32();
        if !S::IS_V5 && addr.is_bit(0) {
            // Unaligned on ARMv4
            (val >> 8) as i8 as i16 as u32
        } else {
            // Aligned
            val.u32()
        }
    }

    /// Read a word from the bus (LE).
    /// If address is unaligned, do LDR/SWP behavior.
    pub fn read_word_ldrswp(&mut self, addr: u32, kind: Access) -> u32 {
        let val = self.read::<u32>(addr, kind);
        if addr & 3 != 0 {
            // Unaligned
            let by = (addr & 3) << 3;
            Cpu::<S>::ror_s0(val, by)
        } else {
            // Aligned
            val
        }
    }
}

impl<S: ArmSystem> Deref for SysWrapper<S> {
    type Target = S;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl<S: ArmSystem> DerefMut for SysWrapper<S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner }
    }
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
