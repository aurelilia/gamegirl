// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ptr;

use crate::numutil::NumExt;

pub trait MemoryMappedSystem<const SIZE: usize>: Sized {
    type Usize: NumExt;
    const ADDR_MASK: &'static [usize];
    const PAGE_POW: usize;
    const MASK_POW: usize;

    fn get_mapper(&self) -> &MemoryMapper<SIZE>;
    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<SIZE>;
    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryMapper<const SIZE: usize> {
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_pages"))]
    read_pages: [*mut u8; SIZE],
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_pages"))]
    write_pages: [*mut u8; SIZE],
}

impl<const SIZE: usize> MemoryMapper<SIZE> {
    /// Get a value in the mapper. Will try to do a fast read from page tables,
    /// falls back to given closure if no page table is mapped at that address.
    #[inline]
    pub fn get<Sys: MemoryMappedSystem<SIZE>, T>(
        this: &Sys,
        addr: Sys::Usize,
        align: Sys::Usize,
        slow: impl FnOnce(&Sys, Sys::Usize) -> T,
    ) -> T {
        let aligned = addr & align;
        let ptr = this.get_mapper().page::<Sys, false>(aligned);
        if ptr as usize > (1 << Sys::PAGE_POW) {
            unsafe { (ptr as *const T).read() }
        } else {
            slow(this, addr)
        }
    }

    /// Sets a value in the mapper. Will try to do a fast write with page
    /// tables, falls back to given closure if no page table is mapped at
    /// that address.
    #[inline]
    pub fn set<Sys: MemoryMappedSystem<SIZE>, T>(
        this: &mut Sys,
        addr: Sys::Usize,
        value: T,
        slow: impl FnOnce(&mut Sys, Sys::Usize, T),
    ) {
        let ptr = this.get_mapper().page::<Sys, true>(addr);
        if ptr as usize > (1 << Sys::PAGE_POW) {
            unsafe { ptr::write(ptr.cast(), value) }
        } else {
            slow(this, addr, value);
        }
    }

    /// Get the page table at the given address. Can be a write or read table,
    /// see const generic parameter. If there is no page mapped, returns a
    /// pointer in range `0..(1 << Sys::mask_pow())` (due to offsets to the
    /// (null) pointer)
    #[inline]
    pub fn page<Sys: MemoryMappedSystem<SIZE>, const WRITE: bool>(
        &self,
        addr: Sys::Usize,
    ) -> *mut u8 {
        let masks = Sys::ADDR_MASK;
        let addr = addr.us();
        unsafe {
            let mask = masks.get_unchecked((addr >> Sys::MASK_POW) & (masks.len() - 1));
            let page_idx = (addr >> Sys::PAGE_POW) & (SIZE - 1);
            let page = if WRITE {
                self.write_pages.get_unchecked(page_idx)
            } else {
                self.read_pages.get_unchecked(page_idx)
            };
            page.add(addr & mask)
        }
    }

    pub fn init_pages<Sys: MemoryMappedSystem<SIZE>>(this: &mut Sys) {
        for i in 0..SIZE {
            this.get_mapper_mut().read_pages[i] =
                unsafe { this.get_page::<true>(i * (1 << Sys::PAGE_POW)) };
            this.get_mapper_mut().write_pages[i] =
                unsafe { this.get_page::<false>(i * (1 << Sys::PAGE_POW)) };
        }
    }
}

impl<const SIZE: usize> Default for MemoryMapper<SIZE> {
    fn default() -> Self {
        Self {
            read_pages: serde_pages(),
            write_pages: serde_pages(),
        }
    }
}

fn serde_pages<const SIZE: usize>() -> [*mut u8; SIZE] {
    [ptr::null::<u8>() as *mut u8; SIZE]
}
