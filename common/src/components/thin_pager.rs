// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::{vec, vec::Vec};
use core::{iter, ops::Range, ptr};

use crate::{numutil::NumExt, UnsafeArc};

pub const RW: u8 = 0;
pub const RO: u8 = 1 << 0;

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThinPager {
    #[cfg_attr(feature = "serde", serde(skip, default))]
    pub pages: UnsafeArc<Vec<Page>>,
}

impl ThinPager {
    pub fn init(&mut self, max_addr: u32) {
        let len = Self::addr_to_page(max_addr) + 1;
        *self.pages = vec![Page::default(); len];
    }

    pub fn map(&mut self, slice: &[u8], range: Range<u32>, flags: u8) {
        debug_assert!(range.len() % slice.len() == 0);
        debug_assert!(slice.len() & 0x3FFF == 0);

        if slice.len().is_power_of_two() {
            self.map_ptr(slice.as_ptr() as *mut _, range, slice.len() - 1, flags);
        } else {
            let repeats = range.len() / slice.len();
            let mut range = (range.start)..(range.start + slice.len().u32());
            for _ in 0..repeats {
                self.map_ptr(slice.as_ptr() as *mut _, range.clone(), usize::MAX, flags);
                range.start += slice.len().u32();
                range.end += slice.len().u32();
            }
        }
    }

    pub fn map_ptr(&mut self, ptr: *mut u8, range: Range<u32>, mask: usize, flags: u8) {
        for (idx, page) in self.pages[Self::addr_to_page_range(range)]
            .iter_mut()
            .enumerate()
        {
            page.ptr = unsafe { ptr.byte_add((idx * 0x4000) & mask) };
            page.flags = flags;
        }
    }

    pub fn evict(&mut self, range: Range<u32>) {
        for page in &mut self.pages[Self::addr_to_page_range(range)] {
            page.ptr = ptr::null_mut();
        }
    }

    pub fn read<T: Copy>(&self, addr: u32) -> Option<T> {
        let ptr = self.raw_read(addr) as *const T;
        (ptr as usize >= 0x4000).then(|| unsafe { *ptr })
    }

    pub fn write<T: Copy>(&mut self, addr: u32) -> Option<&mut T> {
        let ptr = self.raw_write(addr) as *mut T;
        (ptr as usize >= 0x4000).then(|| unsafe { &mut *ptr })
    }

    fn raw_read(&self, addr: u32) -> *const u8 {
        let page = &self.pages[Self::addr_to_page(addr)];
        unsafe { page.ptr.byte_add(addr as usize & 0x3FFF) }
    }

    fn raw_write(&mut self, addr: u32) -> *mut u8 {
        let page = self.get_raw(addr);
        if page.flags & RO != 0 {
            ptr::null_mut()
        } else {
            unsafe { page.ptr.byte_add(addr as usize & 0x3FFF) }
        }
    }

    pub fn get_raw(&mut self, addr: u32) -> &mut Page {
        &mut self.pages[Self::addr_to_page(addr)]
    }

    pub fn normalize(v: &mut Vec<u8>) {
        let until_full_page = 0x4000 - (v.len() & 0x3FFF);
        v.extend(iter::repeat(0).take(until_full_page));
    }

    pub fn is_page_boundary(addr: u32) -> bool {
        addr & 0x3FFF == 0
    }

    pub fn addr_to_page(addr: u32) -> usize {
        addr.us() >> 14
    }

    fn addr_to_page_range(addr: Range<u32>) -> Range<usize> {
        Self::addr_to_page(addr.start)..Self::addr_to_page(addr.end)
    }
}

#[derive(Clone)]
pub struct Page {
    pub ptr: *mut u8,
    pub flags: u8,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            flags: 0,
        }
    }
}

unsafe impl Send for Page {}
unsafe impl Sync for Page {}

#[cfg(test)]
mod test {
    use alloc::vec;

    use super::*;

    #[test]
    fn test_basic() {
        let mut tp = ThinPager::default();
        tp.init(0x8000);
        let v = vec![0; 0x4000];
        tp.map(&v, 0..0x4000, RW);
        tp.map(&v, 0x4000..0x8000, RO);
        assert_eq!(tp.read::<u32>(0), Some(0));
        assert_eq!(tp.write::<u32>(0), Some(&mut 0));
        assert_eq!(tp.read::<u32>(0x4000), Some(0));
        assert_eq!(tp.write::<u32>(0x4000), None);
    }

    #[test]
    fn test_map_ptr() {
        let mut tp = ThinPager::default();
        tp.init(0x8000);
        let v = vec![0; 0x4000];
        tp.map_ptr(v.as_ptr() as *mut _, 0..0x4000, usize::MAX, RW);
        tp.map_ptr(v.as_ptr() as *mut _, 0x4000..0x8000, usize::MAX, RO);
        assert_eq!(tp.read::<u32>(0), Some(0));
        assert_eq!(tp.write::<u32>(0), Some(&mut 0));
        assert_eq!(tp.read::<u32>(0x4000), Some(0));
        assert_eq!(tp.write::<u32>(0x4000), None);
    }

    #[test]
    fn test_evict() {
        let mut tp = ThinPager::default();
        tp.init(0x8000);
        let v = vec![0; 0x4000];
        tp.map(&v, 0..0x4000, RW);
        tp.map(&v, 0x4000..0x8000, RO);
        tp.evict(0..0x4000);
        assert_eq!(tp.read::<u32>(0), None);
        assert_eq!(tp.write::<u32>(0), None);
        assert_eq!(tp.read::<u32>(0x4000), Some(0));
        assert_eq!(tp.write::<u32>(0x4000), None);
    }
}
