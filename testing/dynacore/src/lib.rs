// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::Mutex;

pub use common;
use common::{misc::SystemConfig, Colour, Core};
pub use gamegirl;
use gamegirl::dummy_core;
use once_cell::sync::Lazy;

static CORE: Lazy<Mutex<Box<dyn Core>>> = Lazy::new(|| Mutex::new(dummy_core()));

#[repr(C)]
pub struct DynCore {
    pub load: fn(Vec<u8>),
    pub destroy: fn(),

    pub advance_once: fn(),
    pub advance_delta: fn(f32),

    pub last_frame: fn() -> Option<Vec<Colour>>,
    pub screen_size: fn() -> [usize; 2],
    pub get_address: fn(usize) -> u8,
    pub get_registers: fn() -> Vec<usize>,
    pub get_serial: fn() -> Vec<u8>,
}

// We allow this here, since the library is only meant to be consumed by
// the testbench; which is compiled by the same version of the compiler
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn init() -> DynCore {
    DynCore {
        load: |cart| {
            *CORE.lock().unwrap() =
                gamegirl::load_cart(cart, None, &SystemConfig::default(), None, 0)
        },
        destroy: || *CORE.lock().unwrap() = dummy_core(),

        advance_once: || CORE.lock().unwrap().advance(),
        advance_delta: |delta| CORE.lock().unwrap().advance_delta(delta),

        last_frame: || CORE.lock().unwrap().last_frame(),
        screen_size: || CORE.lock().unwrap().screen_size(),
        get_address: |addr| CORE.lock().unwrap().get_memory(addr),
        get_registers: || CORE.lock().unwrap().get_registers(),
        get_serial: || CORE.lock().unwrap().get_serial(),
    }
}
