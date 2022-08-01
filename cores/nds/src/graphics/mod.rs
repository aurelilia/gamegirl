// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::Colour;
use gga::graphics::threading::{new_ppu, GgaPpu};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct NdsEngines {
    pub ppus: [GgaPpu; 2],
    pub last_frame: Option<Vec<Colour>>,
}

impl Default for NdsEngines {
    fn default() -> Self {
        Self {
            ppus: [new_ppu(), new_ppu()],
            last_frame: None,
        }
    }
}

#[cfg(feature = "threaded-ppu")]
mod thread {
    use std::sync::MutexGuard;

    use gga::graphics::Ppu;

    use crate::Nds;

    impl Nds {
        #[inline]
        pub fn ppu<const E: usize>(&mut self) -> MutexGuard<Ppu> {
            self.ppu.ppus[E].ppu.lock().unwrap()
        }

        #[inline]
        pub fn ppu_nomut<const E: usize>(&self) -> MutexGuard<Ppu> {
            self.ppu.ppus[E].ppu.lock().unwrap()
        }
    }
}

#[cfg(not(feature = "threaded-ppu"))]
mod thread {
    use gga::graphics::Ppu;

    use crate::Nds;

    impl Nds {
        #[inline]
        pub fn ppu<const E: usize>(&mut self) -> &mut Ppu {
            &mut self.ppu.ppus[E]
        }

        #[inline]
        pub fn ppu_nomut<const E: usize>(&self) -> &Ppu {
            &self.ppu.ppus[E]
        }
    }
}
