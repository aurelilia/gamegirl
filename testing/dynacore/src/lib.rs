// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

pub use common;
use common::{misc::SystemConfig, Core};
pub use gamegirl;

// We allow this here, since the library is only meant to be consumed by
// the testbench; which is compiled by the same version of the compiler
#[allow(improper_ctypes_definitions)]
pub type NewCoreFn = extern "C" fn(Vec<u8>) -> Box<dyn Core>;

// We allow this here, since the library is only meant to be consumed by
// the testbench; which is compiled by the same version of the compiler
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn new_core(cart: Vec<u8>) -> Box<dyn Core> {
    gamegirl::load_cart(cart, None, &SystemConfig::default(), None, 0)
}
