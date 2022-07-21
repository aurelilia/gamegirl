// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::{Deref, DerefMut, IndexMut};

use super::Exception;
use crate::{
    components::arm::{inst_arm::ArmLut, inst_thumb::ThumbLut, Access, Cpu},
    numutil::NumExt,
};

pub trait ArmSystem: IndexMut<u32, Output = u16> + Sized + 'static {
    const ARM_LUT: ArmLut<Self>;
    const THUMB_LUT: ThumbLut<Self>;
    const IE_ADDR: u32;
    const IF_ADDR: u32;
    const IME_ADDR: u32;

    fn cpur(&self) -> &Cpu<Self>;
    fn cpu(&mut self) -> &mut Cpu<Self>;

    fn advance_clock(&mut self);
    fn add_sn_cycles(&mut self, cycles: u16);
    fn add_i_cycles(&mut self, cycles: u16);

    fn exception_happened(&mut self, kind: Exception);
    fn pipeline_stalled(&mut self);

    fn get<T: RwType>(&mut self, addr: u32) -> T;
    fn set<T: RwType>(&mut self, addr: u32, value: T);
    fn wait_time<T: RwType>(&mut self, addr: u32, access: Access) -> u16;

    fn read<T: RwType>(&mut self, addr: u32, access: Access) -> T::ReadOutput {
        let time = self.wait_time::<T>(addr, access);
        self.add_sn_cycles(time);
        let value = self.get::<T>(addr).u32();
        T::ReadOutput::from_u32(if T::WIDTH == 2 {
            // Special handling for halfwords
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
    fn write<T: RwType>(&mut self, addr: u32, value: T, access: Access) {
        let time = self.wait_time::<T>(addr, access);
        self.add_sn_cycles(time);
        self.set(addr, value);
    }

    fn check_debugger(&mut self) -> bool;
    fn can_cache_at(addr: u32) -> bool;
}

#[repr(transparent)]
pub struct SysWrapper<S: ArmSystem> {
    pub inner: *mut S,
}

impl<S: ArmSystem> SysWrapper<S> {
    /// Read a half-word from the bus (LE).
    /// If address is unaligned, do LDRSH behavior.
    pub(crate) fn read_hword_ldrsh(&mut self, addr: u32, kind: Access) -> u32 {
        let time = self.wait_time::<u16>(addr, kind);
        self.add_sn_cycles(time);
        let val = self.get::<u16>(addr).u32();
        if addr.is_bit(0) {
            // Unaligned
            (val >> 8) as i8 as i16 as u32
        } else {
            // Aligned
            val.u32()
        }
    }

    /// Read a word from the bus (LE).
    /// If address is unaligned, do LDR/SWP behavior.
    pub(crate) fn read_word_ldrswp(&mut self, addr: u32, kind: Access) -> u32 {
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

pub trait RwType: NumExt + 'static {
    type ReadOutput: RwType;
}

impl RwType for u8 {
    type ReadOutput = Self;
}

impl RwType for u16 {
    type ReadOutput = u32;
}

impl RwType for u32 {
    type ReadOutput = Self;
}
