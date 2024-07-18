// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ops::BitAnd};

/// Trait for common number operations.
pub trait NumExt: BitAnd<Output = Self> + Copy + PartialEq + Default {
    const WIDTH: u32;

    /// Get the state of the given bit. Returns 0/1.
    fn bit(self, bit: u16) -> Self;
    /// Is the given bit set?
    fn is_bit(&self, bit: u16) -> bool;
    /// Set the given bit.
    fn set_bit(self, bit: u16, state: bool) -> Self;
    /// Convert to u8
    fn u8(self) -> u8;
    /// Convert to u16
    fn u16(self) -> u16;
    /// Convert to u32
    fn u32(self) -> u32;
    /// Convert to usize
    fn us(self) -> usize;
    /// Assert this to be a certain width
    fn assert_width(self, w: u32) -> Self;

    /// Convert from u8
    fn from_u8(from: u8) -> Self;
    /// Convert from u16
    fn from_u16(from: u16) -> Self;
    /// Convert from u32
    fn from_u32(from: u32) -> Self;

    /// Get bits in a certain range
    fn bits(self, start: Self, len: Self) -> Self;

    /// Shift to the left, giving 0 if it does not fit.
    fn wshl(self, by: u32) -> Self;
    /// Shift to the right, giving 0 if it does not fit.
    fn wshr(self, by: u32) -> Self;
}

macro_rules! num_ext_impl {
    ($ty:ident, $w:expr) => {
        impl NumExt for $ty {
            const WIDTH: u32 = $w;

            #[inline(always)]
            fn bit(self, bit: u16) -> $ty {
                ((self >> bit) & 1)
            }

            #[inline(always)]
            fn is_bit(&self, bit: u16) -> bool {
                (self & (1 << bit)) != 0
            }

            #[inline(always)]
            fn set_bit(self, bit: u16, state: bool) -> $ty {
                (self & ((1 << bit) ^ Self::MAX)) | ((state as $ty) << bit)
            }

            #[inline(always)]
            fn u8(self) -> u8 {
                self as u8
            }

            #[inline(always)]
            fn u16(self) -> u16 {
                self as u16
            }

            #[inline(always)]
            fn u32(self) -> u32 {
                self as u32
            }

            #[inline(always)]
            fn us(self) -> usize {
                self as usize
            }

            #[inline(always)]
            fn assert_width(self, w: u32) -> Self {
                assert_eq!($w, w, "Unexpected width!");
                self
            }

            #[inline(always)]
            fn from_u8(from: u8) -> Self {
                from as $ty
            }

            #[inline(always)]
            fn from_u16(from: u16) -> Self {
                from as $ty
            }

            #[inline(always)]
            fn from_u32(from: u32) -> Self {
                from as $ty
            }

            #[inline(always)]
            fn bits(self, start: $ty, len: $ty) -> $ty {
                (self >> start) & ((1 << len) - 1)
            }

            #[inline(always)]
            fn wshl(self, by: u32) -> $ty {
                self.checked_shl(by).unwrap_or(0)
            }

            #[inline(always)]
            fn wshr(self, by: u32) -> $ty {
                self.checked_shr(by).unwrap_or(0)
            }
        }
    };
}

num_ext_impl!(u8, 1);
num_ext_impl!(u16, 2);
num_ext_impl!(u32, 4);
num_ext_impl!(u64, 8);
num_ext_impl!(usize, 8);

// Traits and functions for some more common operations used mainly on GGA.
#[inline(always)]
pub fn hword(lo: u8, hi: u8) -> u16 {
    ((hi as u16) << 8) | lo as u16
}

#[inline(always)]
pub fn word(lo: u16, hi: u16) -> u32 {
    ((hi as u32) << 16) | lo as u32
}

pub trait U16Ext {
    fn low(self) -> u8;
    fn high(self) -> u8;
    fn set_low(self, low: u8) -> u16;
    fn set_high(self, high: u8) -> u16;
    fn i10(self) -> i16;
}

impl U16Ext for u16 {
    #[inline(always)]
    fn low(self) -> u8 {
        self.u8()
    }

    #[inline(always)]
    fn high(self) -> u8 {
        (self >> 8).u8()
    }

    #[inline(always)]
    fn set_low(self, low: u8) -> u16 {
        (self & 0xFF00) | low.u16()
    }

    #[inline(always)]
    fn set_high(self, high: u8) -> u16 {
        (self & 0x00FF) | (high.u16() << 8)
    }

    #[inline(always)]
    fn i10(self) -> i16 {
        let mut result = self & 0x3FF;
        if (self & 0x0400) > 1 {
            result |= 0xFC00;
        }
        result as i16
    }
}

pub trait U32Ext {
    fn low(self) -> u16;
    fn high(self) -> u16;
    fn set_low(self, low: u16) -> u32;
    fn set_high(self, high: u16) -> u32;
    fn i24(self) -> i32;
}

impl U32Ext for u32 {
    #[inline(always)]
    fn low(self) -> u16 {
        self.u16()
    }

    #[inline(always)]
    fn high(self) -> u16 {
        (self >> 16).u16()
    }

    #[inline(always)]
    fn set_low(self, low: u16) -> u32 {
        (self & 0xFFFF_0000) | low.u32()
    }

    #[inline(always)]
    fn set_high(self, high: u16) -> u32 {
        (self & 0x0000_FFFF) | (high.u32() << 16)
    }

    #[inline(always)]
    fn i24(self) -> i32 {
        ((self.bits(0, 24) << 8) as i32) >> 8
    }
}

pub trait ByteArrayExt {
    fn get_exact<T>(&self, addr: usize) -> T;
    fn get_wrap<T>(&self, addr: usize) -> T;
    fn try_get_exact<T: NumExt>(&self, addr: usize) -> Option<T>;
    fn set_exact<T>(&self, addr: usize, value: T);
    fn set_wrap<T>(&self, addr: usize, value: T);
}

impl ByteArrayExt for [u8] {
    fn get_exact<T>(&self, addr: usize) -> T {
        unsafe { inner_exact::<T>(self, addr).read() }
    }

    fn get_wrap<T>(&self, addr: usize) -> T {
        unsafe { inner_wrap::<T>(self, addr).read() }
    }

    fn try_get_exact<T: NumExt>(&self, addr: usize) -> Option<T> {
        if (addr + (T::WIDTH.us() - 1)) < self.len() {
            Some(unsafe { inner_exact::<T>(self, addr).read() })
        } else {
            None
        }
    }

    fn set_exact<T>(&self, addr: usize, value: T) {
        unsafe { inner_exact::<T>(self, addr).write(value) }
    }

    fn set_wrap<T>(&self, addr: usize, value: T) {
        unsafe { inner_wrap::<T>(self, addr).write(value) }
    }
}

fn inner_exact<T>(arr: &[u8], addr: usize) -> *mut T {
    assert!((addr + (mem::size_of::<T>() - 1)) < arr.len());
    unsafe { arr.as_ptr().add(addr) as *const T as *mut T }
}
fn inner_wrap<T>(arr: &[u8], addr: usize) -> *mut T {
    debug_assert!(arr.len().is_power_of_two());
    let mask = arr.len() - 1;
    unsafe { arr.as_ptr().add(addr & mask) as *const T as *mut T }
}

pub fn set_u64(dword: u64, idx: u32, hword: u16) -> u64 {
    let mut bytes_in = dword.to_le_bytes();
    let bytes_h = hword.to_le_bytes();
    let idx = idx.us() << 1;
    bytes_in[idx] = bytes_h[0];
    bytes_in[idx + 1] = bytes_h[1];
    u64::from_le_bytes(bytes_in)
}

pub fn get_u64(dword: u64, idx: u32) -> u16 {
    let bytes_in = dword.to_le_bytes();
    let idx = idx.us() << 1;
    u16::from_le_bytes([bytes_in[idx], bytes_in[idx + 1]])
}
