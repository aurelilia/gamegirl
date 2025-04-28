use alloc::{vec, vec::Vec};
use core::marker::PhantomData;

use common::{components::thin_pager::ThinPager, numutil::NumExt};
use waitloop::{WaitloopData, WaitloopPoint};

use crate::{interface::Bus, Address};

pub mod cache;
pub mod waitloop;

pub struct Optimizations<S: Bus> {
    pub waitloop: WaitloopData,
    pub table: OptimizationData<S>,
}

impl<S: Bus> Default for Optimizations<S> {
    fn default() -> Self {
        let mut s = Self {
            waitloop: Default::default(),
            table: OptimizationData { pages: Vec::new() },
        };
        s.table
            .pages
            .resize_with(ThinPager::addr_to_page(0xFFF_FFFF) + 1, || None);
        s
    }
}

pub struct OptimizationData<S: Bus> {
    pages: Vec<Option<PageData<S>>>,
}

impl<S: Bus> OptimizationData<S> {
    fn get_entry(&mut self, addr: Address) -> Option<&mut OptEntry<S>> {
        let page = ThinPager::addr_to_page(addr.0);
        if let Some(Some(page)) = self.pages.get_mut(page.us()) {
            page.entries.get_mut((addr.0.us() & 0x3FFF) >> 1)
        } else {
            None
        }
    }

    fn get_or_create_entry(&mut self, addr: Address) -> Option<&mut OptEntry<S>> {
        let page = ThinPager::addr_to_page(addr.0);
        match self.pages.get_mut(page.us()) {
            Some(Some(page)) => page.entries.get_mut((addr.0.us() & 0x3FFF) >> 1),
            Some(empty_page) => {
                let new = PageData {
                    entries: vec![
                        OptEntry {
                            waitloop: WaitloopPoint::Unanalyzed,
                            _s: PhantomData::default()
                        };
                        0x2000
                    ],
                };
                *empty_page = Some(new);
                empty_page
                    .as_mut()
                    .unwrap()
                    .entries
                    .get_mut((addr.0.us() & 0x3FFF) >> 1)
            }
            None => None,
        }
    }
}

struct PageData<S: Bus> {
    entries: Vec<OptEntry<S>>,
}

struct OptEntry<S: Bus> {
    waitloop: WaitloopPoint,
    _s: PhantomData<S>,
}

impl<S: Bus> Clone for OptEntry<S> {
    fn clone(&self) -> Self {
        Self {
            waitloop: self.waitloop.clone(),
            _s: self._s.clone(),
        }
    }
}
