// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::ptr;

use crate::numutil::NumExt;

/// Trait to be implemented by systems that want fast
/// memory access. Implemented using page tables.
pub trait MemoryMappedSystem<const SIZE: usize>: Sized {
    /// Pointer size of the system.
    type Usize: NumExt;
    /// A set of masks to use when masking the pointer offset
    /// of the page table entry.
    const ADDR_MASK: &'static [usize];
    /// The size of a page, expressed as 2^PAGE_POW.
    const PAGE_POW: usize;
    /// The size of an address mask region in `ADDR_MASK`, expressed as
    /// 2^PAGE_POW.
    const MASK_POW: usize;

    /// Function to return the mapper.
    fn get_mapper(&self) -> &MemoryMapper<SIZE>;
    /// Function to return the mapper.
    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<SIZE>;
    /// Function that should return the page at the given address.
    ///
    /// # Safety
    /// This function is unsafe since it deals with raw pointers,
    /// doing arithmetic. It should not do any more than that
    /// and therefore is expected to be safe to call with any address.
    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryMapper<const SIZE: usize> {
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_pages::<SIZE>"))]
    read_pages: Box<[*mut u8]>,
    #[cfg_attr(feature = "serde", serde(skip))]
    #[cfg_attr(feature = "serde", serde(default = "serde_pages::<SIZE>"))]
    write_pages: Box<[*mut u8]>,
}

impl<const SIZE: usize> MemoryMapper<SIZE> {
    /// Get a value in the mapper. Assumes address to be aligned.
    #[inline]
    pub fn get<Sys: MemoryMappedSystem<SIZE>, T>(&self, addr: Sys::Usize) -> Option<T> {
        let ptr = self.page::<Sys, false>(addr);
        if ptr as usize > (1 << Sys::PAGE_POW) {
            Some(unsafe { (ptr as *const T).read() })
        } else {
            None
        }
    }

    /// Sets a value in the mapper. Returns success.
    #[inline]
    pub fn set<Sys: MemoryMappedSystem<SIZE>, T>(&mut self, addr: Sys::Usize, value: T) -> bool {
        let ptr = self.page::<Sys, true>(addr);
        if ptr as usize > (1 << Sys::PAGE_POW) {
            unsafe { ptr::write(ptr.cast(), value) }
            true
        } else {
            false
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

    /// Initialize page tables based on the pages given by the system.
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
            read_pages: serde_pages::<SIZE>(),
            write_pages: serde_pages::<SIZE>(),
        }
    }
}

unsafe impl<const SIZE: usize> Send for MemoryMapper<SIZE> {}
unsafe impl<const SIZE: usize> Sync for MemoryMapper<SIZE> {}

fn serde_pages<const SIZE: usize>() -> Box<[*mut u8]> {
    Box::new([ptr::null::<u8>() as *mut u8; SIZE])
}
