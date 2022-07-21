// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod memory;
mod scheduling;

use serde::{Deserialize, Serialize};

use crate::{
    common::{EmulateOptions, SystemConfig},
    components::{debugger::Debugger, scheduler::Scheduler},
    gga::{cpu::Cpu, graphics::threading::GgaPpu},
    nds::{memory::Memory, scheduling::NdsEvent},
    numutil::NumExt,
};

#[derive(Deserialize, Serialize)]
pub struct Nds {
    cpu7: Cpu,
    cpu9: Cpu,
    pub ppus: [GgaPpu; 2],
    memory: Memory,
    scheduler: Scheduler<NdsEvent>,

    #[serde(skip)]
    #[serde(default)]
    pub debugger: Debugger<u32>,
    pub options: EmulateOptions,
    pub config: SystemConfig,
    ticking: bool,
}

impl Nds {
    /// Add S/N cycles, which advance the system besides the CPU.
    #[inline]
    fn add_sn_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }

    /// Add I cycles, which advance the system besides the CPU.
    #[inline]
    fn add_i_cycles(&mut self, count: u16) {
        self.scheduler.advance(count.u32());
    }
}
