// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};

use common::{components::thin_pager::ThinPager, numutil::NumExt};

use crate::{
    arm::ArmHandler,
    interface::{ArmSystem, SysWrapper},
    thumb::ThumbHandler,
};

/// Storage for instruction caching.
pub struct Cache<S: ArmSystem> {
    pages: Vec<Option<Box<PageData<S>>>>,
    pub enabled: bool,
}

impl<S: ArmSystem> Cache<S> {
    /// Get the cache at the given location, if available.
    pub fn get(&self, pc: u32) -> Option<CacheEntry<S>> {
        let page = ThinPager::addr_to_page(pc);
        if let Some(Some(page)) = self.pages.get(page.us()) {
            page.entries.get((pc.us() & 0x3FFF) >> 1).cloned().flatten()
        } else {
            None
        }
    }

    /// Invalidate all caches in the given page.
    pub fn invalidate_address(&mut self, pc: u32) {
        if !self.enabled || pc > 0xFFF_FFFF {
            return;
        }

        let page = ThinPager::addr_to_page(pc);
        self.pages[page] = None;
    }

    /// Put a cache at the given PC.
    pub fn put(&mut self, pc: u32, entry: CacheEntry<S>) {
        if !self.enabled {
            return;
        }

        let slot = ThinPager::addr_to_page(pc);
        let location = (pc & 0x3FFF) >> 1;
        if let Some(page) = &mut self.pages[slot.us()] {
            Cache::insert(&mut page.entries, location, entry);
        } else {
            let mut page = Box::new(PageData {
                entries: vec![None; 0x4000 >> 1],
            });
            Cache::insert(&mut page.entries, location, entry);
            self.pages[slot.us()] = Some(page);
        }
    }

    fn insert(set: &mut [Option<CacheEntry<S>>], location: u32, entry: CacheEntry<S>) {
        set[location.us()] = Some(entry);
    }

    /// Initialize caches.
    pub fn init(&mut self) {
        self.pages.clear();
        self.pages
            .resize_with(ThinPager::addr_to_page(0xFFF_FFFF) + 1, || None);
        self.enabled = true;
    }

    /// If a block should be forcibly ended. True at page boundaries.
    pub fn force_end_block(pc: u32) -> bool {
        ThinPager::is_page_boundary(pc)
    }
}

impl<S: ArmSystem> Default for Cache<S> {
    fn default() -> Self {
        Self {
            pages: Vec::default(),
            enabled: false,
        }
    }
}

#[derive(Clone)]
pub struct PageData<S: ArmSystem> {
    pub entries: Vec<Option<CacheEntry<S>>>,
}

/// Cache entry, ARM or THUMB instructions
pub enum CacheEntry<S: ArmSystem> {
    ArmCache(Arc<Vec<CachedInst<u32, ArmHandler<S>>>>),
    ThumbCache(Arc<Vec<CachedInst<u16, ThumbHandler<SysWrapper<S>>>>>),
}

impl<S: ArmSystem> Clone for CacheEntry<S> {
    fn clone(&self) -> Self {
        match self {
            Self::ArmCache(arg0) => Self::ArmCache(arg0.clone()),
            Self::ThumbCache(arg0) => Self::ThumbCache(arg0.clone()),
        }
    }
}

/// A cached instruction
pub struct CachedInst<I, H> {
    /// The instruction itself, an unsigned integer
    pub inst: I,
    /// The handler to execute for it
    pub handler: H,
    /// The amount of cycles the instruction took
    pub sn_cycles: u16,
}
