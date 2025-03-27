// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::marker::PhantomData;

use crate::numutil::NumExt;

pub const FAILED_READ: (u32, u32, u32) = (0, 0, 1);
pub const FAILED_WRITE: (u32, u32) = (0, 1);

#[macro_export]
macro_rules! io08 {
    ($ma:expr, $addr:expr, $exec: expr) => {{
        if $ma == $addr {
            let exec: u8 = { $exec };
            return (exec as u32, 0, 1);
        }
    }};
}

#[macro_export]
macro_rules! io16 {
    ($ma:expr, $addr:expr, $exec: expr) => {{
        if ($ma & !1) == $addr {
            let exec: u16 = { $exec };
            return (exec as u32, $ma & 1, 2);
        }
    }};
}

#[macro_export]
macro_rules! io32 {
    ($ma:expr, $addr:expr, $exec: expr) => {{
        if ($ma & !3) == $addr {
            return ({ $exec }, $ma & 3, 4);
        }
    }};
}

#[macro_export]
macro_rules! iow08 {
    ($a:expr, $addr:expr, $exec: expr) => {{
        if $a == $addr {
            let _exec: () = { $exec };
            return (0, 1);
        }
    }};
}

#[macro_export]
macro_rules! iow16 {
    ($a:expr, $addr:expr, $exec: expr) => {{
        if ($a & !1) == $addr {
            let _exec: () = { $exec };
            return ($a & 1, 2);
        }
    }};
}

#[macro_export]
macro_rules! iow32 {
    ($a:expr, $addr:expr, $exec: expr) => {{
        if ($a & !3) == $addr {
            let _exec: () = { $exec };
            return ($a & 3, 4);
        }
    }};
}

pub fn get_mmio_apply<T: NumExt>(addr: u32, mut inner: impl FnMut(u32) -> (u32, u32, u32)) -> T {
    let addr = addr & 0xFF_FFFF;
    let mut out = 0;

    let mut current_byte = 0;
    while current_byte < T::WIDTH {
        let (value, from_start_offset, reg_size) = inner(addr + current_byte);
        out |= (value >> (from_start_offset * 8)) << (current_byte * 8);
        current_byte += reg_size - from_start_offset;
    }

    T::from_u32(out)
}

pub fn set_mmio_apply<T: NumExt>(
    addr: u32,
    value: T,
    mut inner: impl FnMut(u32, u32, u32) -> (u32, u32),
) {
    let addr = addr & 0xFF_FFFF;
    let mut value = value.u32();

    let mut current_byte = 0;
    let mut mask = u32::MAX >> ((4 - T::WIDTH) * 8);
    while current_byte < T::WIDTH {
        let (from_start_offset, reg_size) = inner(addr + current_byte, value, mask);
        let written = reg_size - from_start_offset;
        value = value.wrapping_shr(written * 8);
        mask = mask.wrapping_shr(written * 8);
        current_byte += written;
    }
}

pub fn section<T: NumExt>(addr: u32, new: u32, mask: u32) -> IoSection<T> {
    let offs = addr & (T::WIDTH - 1);
    let mask = mask << (offs * 8);
    let value = new << (offs * 8);
    IoSection {
        value,
        mask,
        _ph: PhantomData::default(),
    }
}

#[derive(Copy, Clone)]
pub struct IoSection<T> {
    value: u32,
    mask: u32,
    _ph: PhantomData<T>,
}

impl<T: NumExt> IoSection<T> {
    pub fn apply(&self, to: &mut T) {
        *to = self.with(*to);
    }

    pub fn mask(mut self, mask: u32) -> Self {
        self.mask &= mask;
        self
    }

    pub fn apply_io<E>(&self, to: &mut E)
    where
        T: From<E>,
        E: Copy + From<T>,
    {
        *to = self.with((*to).into()).into();
    }

    pub fn apply_io_ret<E>(&self, to: &mut E) -> E
    where
        T: From<E>,
        E: Copy + From<T>,
    {
        *to = self.with((*to).into()).into();
        *to
    }

    pub fn with(&self, with: T) -> T {
        T::from_u32((with.u32() & !self.mask) | (self.value & self.mask))
    }

    pub fn raw(&self) -> T {
        T::from_u32(self.value & self.mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_apply_keep_all() {
        let mut value = 0x1234_5678;
        let section = section::<u32>(0, 0x8765_4321, 0xFFFF_FFFF);
        section.apply(&mut value);
        assert_eq!(value, 0x8765_4321);
    }

    #[test]
    fn test_section_apply_keep_some() {
        let section = section::<u32>(0, 0x4321, 0x0000_FFFF);
        assert_eq!(section.with(0x1234_5678), 0x1234_4321);
    }

    #[test]
    fn test_get_mmio_single_byte() {
        let value = get_mmio_apply::<u8>(0, |a| {
            io08!(a, 0, 0x12);
            FAILED_READ
        });
        assert_eq!(value, 0x12);
    }

    #[test]
    fn test_get_mmio_single_halfword() {
        let value = get_mmio_apply::<u16>(0, |a| {
            io16!(a, 0, 0x1234);
            FAILED_READ
        });
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_get_mmio_half_halfword_lower() {
        let value = get_mmio_apply::<u16>(1, |a| {
            io16!(a, 0, 0x1234);
            FAILED_READ
        });
        assert_eq!(value, 0x0012);
    }

    #[test]
    fn test_get_mmio_half_halfword_upper() {
        let value = get_mmio_apply::<u16>(0, |a| {
            io08!(a, 1, 0x12);
            FAILED_READ
        });
        assert_eq!(value, 0x1200);
    }

    #[test]
    fn test_get_mmio_nothing() {
        let value = get_mmio_apply::<u16>(0, |_| FAILED_READ);
        assert_eq!(value, 0);
    }

    #[test]
    fn test_get_mmio_cross_register() {
        let value = get_mmio_apply::<u16>(0, |a| {
            io08!(a, 0, 0x12);
            io08!(a, 1, 0x34);
            FAILED_READ
        });
        assert_eq!(value, 0x3412);
    }

    #[test]
    fn test_set_mmio_single_byte() {
        let mut value = 0;
        set_mmio_apply(0, 0x12u8, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            iow08!(a, 0, s8.apply(&mut value));
            FAILED_WRITE
        });
        assert_eq!(value, 0x12);
    }

    #[test]
    fn test_set_mmio_single_halfword() {
        let mut value = 0;
        set_mmio_apply(0, 0x1234u16, |a, v, m| {
            let s16 = section::<u16>(a, v, m);
            iow16!(a, 0, s16.apply(&mut value));
            FAILED_WRITE
        });
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_set_mmio_half_halfword_lower() {
        let mut value = 0;
        set_mmio_apply(1, 0x12u8, |a, v, m| {
            let s16 = section::<u16>(a, v, m);
            iow16!(a, 0, s16.apply(&mut value));
            FAILED_WRITE
        });
        assert_eq!(value, 0x1200);
    }

    #[test]
    fn test_set_mmio_half_halfword_upper() {
        let mut value = 0;
        set_mmio_apply(0, 0x1234u16, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            iow08!(a, 1, s8.apply(&mut value));
            FAILED_WRITE
        });
        assert_eq!(value, 0x0012);
    }

    #[test]
    fn test_set_mmio_cross_register() {
        let mut value1 = 0;
        let mut value2 = 0;
        set_mmio_apply(0, 0x1234u16, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            iow08!(a, 0, s8.apply(&mut value1));
            iow08!(a, 1, s8.apply(&mut value2));
            FAILED_WRITE
        });
        assert_eq!(value2, 0x12);
        assert_eq!(value1, 0x34);
    }
}
