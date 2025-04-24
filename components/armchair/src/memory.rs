use core::{
    fmt::Display,
    ops::{Add, AddAssign, Sub, SubAssign},
};

use common::numutil::NumExt;

use crate::{
    interface::{Bus, CpuVersion, RwType},
    Cpu,
};

#[derive(Debug, Copy, Clone)]
pub struct Address(pub u32);

impl Address {
    pub const BYTE: Address = Address(1);
    pub const HW: Address = Address(2);
    pub const WORD: Address = Address(4);

    pub fn add_rel(self, rel: RelativeOffset) -> Address {
        Address(self.0.wrapping_add_signed(rel.0))
    }

    pub fn add_signed(self, rhs: Address, positive: bool) -> Address {
        if positive {
            self + rhs
        } else {
            self - rhs
        }
    }

    pub fn to_rel(self, up: bool) -> RelativeOffset {
        if up {
            RelativeOffset(self.0 as i32)
        } else {
            RelativeOffset(-(self.0 as i32))
        }
    }
}

impl Add for Address {
    type Output = Address;

    fn add(self, rhs: Self) -> Self::Output {
        Address(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Address {
    type Output = Address;

    fn sub(self, rhs: Self) -> Self::Output {
        Address(self.0.wrapping_sub(rhs.0))
    }
}

impl AddAssign for Address {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Address {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "#0x{:X}", self.0)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RelativeOffset(pub i32);

impl Display for RelativeOffset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 >= 0 {
            write!(f, "#0x{:X}", self.0)
        } else {
            write!(f, "#-0x{:X}", -self.0)
        }
    }
}

/// Enum for the types of memory accesses; either sequential
/// or non-sequential.
pub type Access = u8;

pub mod access {
    use super::Access;
    pub const NONSEQ: Access = 0;
    pub const SEQ: Access = 1 << 0;
    pub const CODE: Access = 1 << 1;
    pub const DMA: Access = 1 << 2;
}

impl<S: Bus> Cpu<S> {
    /// Get the value at the given memory address and add to the system clock.
    pub fn read<T: RwType>(&mut self, addr: Address, access: Access) -> T::ReadOutput {
        let time = self.bus.wait_time::<T>(addr, access);
        self.bus.tick(time as u64);

        let value = self.bus.get::<T>(addr).u32();
        self.waitloop
            .on_read(addr, value.u32(), T::from_u32(u32::MAX).u32());

        T::ReadOutput::from_u32(if !S::Version::IS_V5 && T::WIDTH == 2 {
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
    pub fn write<T: RwType>(&mut self, addr: Address, value: T, access: Access) {
        let time = self.bus.wait_time::<T>(addr, access);
        self.bus.tick(time as u64);
        self.waitloop.on_write();
        self.debugger.write_occurred(addr.0);
        self.bus.set(addr, value);
    }
}
