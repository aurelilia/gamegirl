use alloc::{vec, vec::Vec};
use core::ops::Range;

use analyze::{BlockAnalysis, InstructionAnalyzer};
use cache::{CacheEntry, CacheEntryKind, CodeOptimizationStatus};
use common::{components::thin_pager::ThinPager, numutil::NumExt};
use jit::{Jit, JitBlock};
use waitloop::{WaitloopData, WaitloopPoint};

use crate::{interface::Bus, misc::InstructionKind, Address, Cpu};

pub mod analyze;
pub mod cache;
pub mod jit;
pub mod waitloop;

pub struct Optimizations<S: Bus> {
    pub waitloop: WaitloopData,
    pub code_opt: CodeOptimizationStatus,
    jit: Jit,
    pub table: OptimizationData<S>,
}

impl<S: Bus> Default for Optimizations<S> {
    fn default() -> Self {
        let mut s = Self {
            waitloop: Default::default(),
            code_opt: CodeOptimizationStatus::JustInterpret,
            jit: Jit::new::<S>(),
            table: OptimizationData {
                pages: Vec::new(),
                functions: Vec::new(),
                blocks: Vec::new(),
                caches: Vec::new(),
                jits: Vec::new(),
            },
        };
        s.table
            .pages
            .resize_with(ThinPager::addr_to_page(0xFFF_FFFF) + 1, || None);
        s
    }
}

pub struct OptimizationData<S: Bus> {
    pages: Vec<Option<PageData>>,
    functions: Vec<BlockAnalysis>,
    blocks: Vec<BlockAnalysis>,
    caches: Vec<CacheEntry<S>>,
    jits: Vec<JitBlock>,
}

impl<S: Bus> OptimizationData<S> {
    fn get_or_create_entry(&mut self, addr: Address) -> Option<&mut OptEntry> {
        let page = ThinPager::addr_to_page(addr.0);
        match self.pages.get_mut(page.us()) {
            Some(Some(page)) => page.entries.get_mut((addr.0.us() & 0x3FFF) >> 1),
            Some(empty_page) => {
                let new = PageData {
                    entries: vec![
                        OptEntry {
                            waitloop: WaitloopPoint::Unanalyzed,
                            function_entry_analysis: None,
                            function_exit_analysis: None,
                            block_entry_analysis: None,
                            cache_entry: None,
                            jit_entry: None
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

    pub fn insert_cache(&mut self, index: Option<CacheIndex>, entry: CacheEntryKind<S>) {
        if let Some(index) = index {
            self.caches.insert(index, CacheEntry { entry });
        } else {
            self.caches.push(CacheEntry { entry });
        }
    }

    pub fn get_cache(&mut self, index: CacheIndex) -> &'static CacheEntryKind<S> {
        self.caches[index].borrow()
    }

    pub fn get_jit(&mut self, index: JitIndex) -> JitBlock {
        self.jits[index]
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
    pub(crate) fn on_page_boundary(self) -> bool {
        (self.0 & 0x3FFF) == 0
    }
}

struct PageData {
    entries: Vec<OptEntry>,
}

#[derive(Copy)]
struct OptEntry {
    waitloop: WaitloopPoint,
    cache_entry: Option<CacheIndex>,
    jit_entry: Option<JitIndex>,

    function_entry_analysis: Option<BlockAnalysisIndex>,
    function_exit_analysis: Option<BlockAnalysisIndex>,
    block_entry_analysis: Option<BlockAnalysisIndex>,
}

impl Clone for OptEntry {
    fn clone(&self) -> Self {
        Self {
            waitloop: self.waitloop.clone(),
            cache_entry: None,
            jit_entry: None,
            function_entry_analysis: None,
            function_exit_analysis: None,
            block_entry_analysis: None,
        }
    }
}

pub type BlockAnalysisIndex = usize;
pub type CacheIndex = usize;
pub type JitIndex = usize;

impl<S: Bus> Cpu<S> {
    pub fn just_called_function(&mut self) {
        let entry = self.state.pc();
        let Some(data_at_pc) = self.opt.table.get_or_create_entry(entry) else {
            return;
        };
        match data_at_pc {
            OptEntry {
                jit_entry: Some(index),
                ..
            } => self.opt.code_opt = CodeOptimizationStatus::RunJitNowAt(*index),

            OptEntry {
                function_entry_analysis: Some(fn_index),
                ..
            } => {
                let fn_index = *fn_index;
                let ana = &self.opt.table.functions[fn_index];
                assert_eq!(ana.entry, entry);
                if ana.instructions.len() < 5 || ana.kind != InstructionKind::Thumb {
                    return;
                }
                let index = self.opt.table.jits.len();
                let jit = self
                    .opt
                    .jit
                    .compile(index, &mut self.state, &mut self.bus, ana);
                self.opt.table.jits.push(jit);
                self.opt.table.get_or_create_entry(entry).unwrap().jit_entry = Some(index);
                self.opt.code_opt = CodeOptimizationStatus::RunJitNowAt(index)
            }

            OptEntry {
                function_entry_analysis: None,
                block_entry_analysis: None,
                ..
            } => {
                self.perform_function_analysis();
                self.perform_block_analysis();
            }

            OptEntry {
                function_entry_analysis: None,
                ..
            } => {
                self.perform_function_analysis();
            }
        }
    }

    fn perform_function_analysis(&mut self) {
        let entry = self.state.pc();
        let kind = self.state.current_instruction_type();
        let analysis = match kind {
            InstructionKind::Arm => InstructionAnalyzer::analyze(
                &mut |addr| self.bus.get::<u32>(&mut self.state, addr),
                entry,
                kind,
                false,
            ),

            InstructionKind::Thumb => InstructionAnalyzer::analyze(
                &mut |addr| self.bus.get::<u16>(&mut self.state, addr).u32(),
                entry,
                kind,
                false,
            ),
        };

        let data_at_exit = self.opt.table.get_or_create_entry(analysis.exit).unwrap();
        if let Some(index) = data_at_exit.function_exit_analysis.and(None) {
            // We have seen this exit before!
            // Use the index of the existing analysis for the current location.
            self.opt
                .table
                .get_or_create_entry(entry)
                .unwrap()
                .function_entry_analysis = Some(index);

            let prev_analysis = &mut self.opt.table.functions[index];
            if prev_analysis.entry <= entry {
                // Previous analysis found an earlier entry point already, we should do nothing.
                return;
            } else {
                // We found an earlier entry, overwrite existing analysis.
                log::debug!(
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
            log::debug!(
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
                .function_entry_analysis = Some(index);
            self.opt
                .table
                .get_or_create_entry(analysis.exit)
                .unwrap()
                .function_exit_analysis = Some(index);
            self.opt.table.functions.push(analysis);
        }
    }

    /// To be called right after a jump, to perform block analysis.
    pub(crate) fn just_jumped(&mut self) {
        let entry = self.state.pc();
        let Some(data_at_pc) = self.opt.table.get_or_create_entry(entry) else {
            return;
        };
        match (data_at_pc.block_entry_analysis, data_at_pc.cache_entry) {
            // (_, Some(index)) => self.opt.cache = CacheStatus::RunCacheNowAt(index), TODO: This
            // does not work yet
            (None, None) => self.perform_block_analysis(),
            _ => (),
        }
    }

    fn perform_block_analysis(&mut self) {
        let entry = self.state.pc();
        let kind = self.state.current_instruction_type();
        let mut bus = |addr| self.bus.get::<u32>(&mut self.state, addr);

        let analysis = InstructionAnalyzer::analyze(&mut bus, entry, kind, true);
        let entry = analysis.entry;
        let block_len = analysis.instructions.len();
        let index = self.opt.table.blocks.len();
        let cache_index = self.opt.table.caches.len();

        self.opt.table.blocks.push(analysis);
        let entry = self.opt.table.get_or_create_entry(entry).unwrap();
        entry.block_entry_analysis = Some(index);

        if block_len > 5 {
            entry.cache_entry = Some(cache_index);
            // self.opt.cache = CacheStatus::MakeCacheNow; TODO
        }
    }
}
