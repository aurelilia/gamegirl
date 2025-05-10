use alloc::{vec, vec::Vec};
use core::ops::Range;

use analyze::{BlockAnalysis, InstructionAnalyzer};
use common::{components::thin_pager::ThinPager, numutil::NumExt};
use jit::{Jit, JitBlock};
use waitloop::{WaitloopData, WaitloopPoint};

use crate::{interface::Bus, misc::InstructionKind, Address, Cpu};

pub mod analyze;
pub mod jit;
pub mod waitloop;

pub struct Optimizations {
    pub waitloop: WaitloopData,
    pub jit_block: Option<JitIndex>,
    jit_ctx: Jit,
    pub table: OptimizationData,
}

impl Optimizations {
    pub fn new<S: Bus>() -> Self {
        let mut s = Self {
            waitloop: Default::default(),
            jit_block: None,
            jit_ctx: Jit::new::<S>(),
            table: OptimizationData {
                pages: Vec::new(),
                analyses: Vec::new(),
                jits: Vec::new(),
            },
        };
        s.table
            .pages
            .resize_with(ThinPager::addr_to_page(0xFFF_FFFF) + 1, || None);
        s
    }
}

pub struct OptimizationData {
    pages: Vec<Option<PageData>>,
    pub analyses: Vec<BlockAnalysis>,
    jits: Vec<JitBlock>,
}

impl OptimizationData {
    fn get_or_create_entry(&mut self, addr: Address) -> Option<&mut OptEntry> {
        let page = ThinPager::addr_to_page(addr.0);
        match self.pages.get_mut(page.us()) {
            Some(Some(page)) => page.entries.get_mut((addr.0.us() & 0x3FFF) >> 1),
            Some(empty_page) => {
                let new = PageData {
                    entries: vec![
                        OptEntry {
                            waitloop: WaitloopPoint::Unanalyzed,
                            entry_analysis: None,
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

    pub fn get_jit(&mut self, index: JitIndex) -> JitBlock {
        self.jits[index]
    }

    pub fn invalidate_address(&mut self, addr: Address) {
        let page = ThinPager::addr_to_page(addr.0);
        self.pages[page] = None; // TODO Be smarter about this
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
    jit_entry: Option<JitIndex>,
    entry_analysis: Option<BlockAnalysisIndex>,
}

impl Clone for OptEntry {
    fn clone(&self) -> Self {
        Self {
            waitloop: self.waitloop.clone(),
            jit_entry: None,
            entry_analysis: None,
        }
    }
}

pub type BlockAnalysisIndex = usize;
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
            } => self.opt.jit_block = Some(*index),

            OptEntry {
                entry_analysis: Some(fn_index),
                ..
            } => {
                let fn_index = *fn_index;
                let ana = &self.opt.table.analyses[fn_index];
                if ana.instructions.len() < 5 || !ana.pure || ana.kind == InstructionKind::Arm {
                    return;
                }

                let index = self.opt.table.jits.len();
                let jit = self
                    .opt
                    .jit_ctx
                    .compile(index, &mut self.state, &mut self.bus, ana);
                self.opt.table.jits.push(jit);
                self.opt.table.get_or_create_entry(entry).unwrap().jit_entry = Some(index);
                self.opt.jit_block = Some(index)
            }

            OptEntry {
                entry_analysis: None,
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
            ),

            InstructionKind::Thumb => InstructionAnalyzer::analyze(
                &mut |addr| self.bus.get::<u16>(&mut self.state, addr).u32(),
                entry,
                kind,
            ),
        };

        log::debug!(
            "Analysis concluded with function from {}-{}, length {}.",
            analysis.entry,
            analysis.exit,
            analysis.instructions.len()
        );
        let index = self.opt.table.analyses.len();
        self.opt
            .table
            .get_or_create_entry(analysis.entry)
            .unwrap()
            .entry_analysis = Some(index);
        self.opt.table.analyses.push(analysis);
    }
}
