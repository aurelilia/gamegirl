use core::{
    fmt::Display,
    ops::{Add, AddAssign, Neg, Sub, SubAssign},
};

use common::numutil::NumExt;

use crate::{
    interface::{Bus, RwType},
    Cpu,
};

#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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

    pub fn align(self, to: u32) -> Address {
        Address(self.0 & !(to - 1))
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
        write!(f, "$0x{:X}", self.0)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RelativeOffset(pub i32);

impl RelativeOffset {
    pub const BYTE: RelativeOffset = RelativeOffset(1);
    pub const HW: RelativeOffset = RelativeOffset(2);
    pub const WORD: RelativeOffset = RelativeOffset(4);

    pub fn mul(self, by: i32) -> RelativeOffset {
        RelativeOffset(self.0 * by)
    }
}

impl Display for RelativeOffset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 >= 0 {
            write!(f, "$0x{:X}", self.0)
        } else {
            write!(f, "$-0x{:X}", -self.0)
        }
    }
}

impl Neg for RelativeOffset {
    type Output = Self;

    fn neg(self) -> Self::Output {
        RelativeOffset(-self.0)
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
        let value = self.bus.read::<T>(&mut self.state, addr, access);
        self.opt
            .waitloop
            .on_read(addr, value.u32(), &mut self.opt.table);
        value
    }

    /// Set the value at the given memory address and add to the system clock.
    pub fn write<T: RwType>(&mut self, addr: Address, value: T, access: Access) {
        self.opt.waitloop.on_write(&mut self.opt.table);
        self.bus.write(&mut self.state, addr, value, access);
    }
}
