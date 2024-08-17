// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use cranelift::CraneliftJit;

use crate::{interface::ArmSystem, optimizations::caching::CacheEntry};

mod cranelift;
mod thumb;

pub struct CompileCtx {
    ctx: CraneliftJit,
}

impl CompileCtx {
    pub fn compile(&mut self, block: CacheEntry<impl ArmSystem>) -> *const u8 {
        todo!();
    }

    pub fn new() -> Self {
        Self {
            ctx: CraneliftJit::default(),
        }
    }
}
