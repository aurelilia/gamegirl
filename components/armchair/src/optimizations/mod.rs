use alloc::{vec, vec::Vec};
use core::{marker::PhantomData, ops::Range};

use analyze::{BlockAnalysis, InstructionAnalyzer};
use common::{components::thin_pager::ThinPager, numutil::NumExt};
use waitloop::{WaitloopData, WaitloopPoint};

use crate::{interface::Bus, Address, Cpu};

pub mod analyze;
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
            table: OptimizationData {
                pages: Vec::new(),
                functions: Vec::new(),
            },
        };
        s.table
            .pages
            .resize_with(ThinPager::addr_to_page(0xFFF_FFFF) + 1, || None);
        s
    }
}

pub struct OptimizationData<S: Bus> {
    pages: Vec<Option<PageData<S>>>,
    functions: Vec<BlockAnalysis>,
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
                            entry_analysis: None,
                            exit_analysis: None,
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

    pub fn invalidate_address(&mut self, addr: Address) {
        let page = ThinPager::addr_to_page(addr.0);
        self.pages[page] = None;
    }

    pub fn invalidate_address_range(&mut self, addrs: Range<u32>) {
        for entry in ThinPager::addr_to_page_range(addrs) {
            self.pages[entry] = None;
        }
    }
}

impl Address {
    fn on_page_boundary(self) -> bool {
        (self.0 & 0x3FFF) == 0
    }
}

struct PageData<S: Bus> {
    entries: Vec<OptEntry<S>>,
}

#[derive(Copy)]
struct OptEntry<S: Bus> {
    waitloop: WaitloopPoint,
    entry_analysis: Option<BlockAnalysisIndex>,
    exit_analysis: Option<BlockAnalysisIndex>,
    _s: PhantomData<S>,
}

impl<S: Bus> Clone for OptEntry<S> {
    fn clone(&self) -> Self {
        Self {
            waitloop: self.waitloop.clone(),
            entry_analysis: None,
            exit_analysis: None,
            _s: self._s.clone(),
        }
    }
}

pub type BlockAnalysisIndex = usize;

impl<S: Bus> Cpu<S> {
    pub fn analyze_now(&mut self) {
        let entry = self.state.pc();
        let kind = self.state.current_instruction_type();
        let mut bus = |addr| self.bus.get::<u32>(&mut self.state, addr);
        let Some(data_at_pc) = self.opt.table.get_or_create_entry(entry) else {
            return;
        };
        if data_at_pc.entry_analysis.is_some() {
            return;
        }

        let analysis = InstructionAnalyzer::analyze(&mut bus, entry, kind);
        let data_at_exit = self.opt.table.get_or_create_entry(analysis.exit).unwrap();

        if let Some(index) = data_at_exit.exit_analysis {
            // We have seen this exit before!
            // Use the index of the existing analysis for the current location.
            self.opt
                .table
                .get_or_create_entry(entry)
                .unwrap()
                .entry_analysis = Some(index);

            let prev_analysis = &mut self.opt.table.functions[index];
            if prev_analysis.entry <= entry {
                // Previous analysis found an earlier entry point already, we should do nothing.
                return;
            } else {
                // We found an earlier entry, overwrite existing analysis.
                log::error!(
                    "Analysis found earlier function start at {} instead of {}; until {}; length {}.",
                    analysis.entry,
                    prev_analysis.entry,
                    analysis.exit,
                    analysis.instructions.len()
                );
                *prev_analysis = analysis;
            }
        } else {
            // We have never seen this exit, its a new function.
            log::error!(
                "Analysis concluded with function from {}-{}, length {}.",
                analysis.entry,
                analysis.exit,
                analysis.instructions.len()
            );
            let index = self.opt.table.functions.len();
            self.opt
                .table
                .get_or_create_entry(analysis.entry)
                .unwrap()
                .entry_analysis = Some(index);
            self.opt
                .table
                .get_or_create_entry(analysis.exit)
                .unwrap()
                .exit_analysis = Some(index);
            self.opt.table.functions.push(analysis);
        }
    }
}
