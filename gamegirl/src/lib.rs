// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! This crate contains common structures shared by all systems.

use std::path::PathBuf;

pub use common::{self, Core};
use common::{
    components::storage::Storage,
    misc::{EmulateOptions, SystemConfig},
};
#[cfg(feature = "gga")]
pub use gga;
#[cfg(feature = "ggc")]
pub use ggc;
#[cfg(feature = "nds")]
pub use nds;
#[cfg(feature = "psx")]
pub use psx;

#[cfg(all(feature = "remote-debugger", target_family = "unix"))]
pub mod remote_debugger;

/// Save a game to disk.
pub fn save_game(system: &dyn Core, path: Option<PathBuf>) {
    let save = system.make_save();
    if let Some(save) = save {
        Storage::save(path, save);
    }
}

/// Load a cart. Tries to automatically pick the right system kind.
pub fn load_cart(cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) -> Box<dyn Core> {
    // We detect GG(C) carts by the first 2 bytes of the "Nintendo" logo header
    // that is present on every cartridge.
    let _is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
    // We detect GGA carts by a zero-filled header region
    let _is_gga = cart.iter().skip(0xB5).take(6).all(|b| *b == 0);
    // We detect NDS carts by a zero-filled header region
    let _is_nds = cart.iter().skip(0x15).take(6).all(|b| *b == 0);

    let mut sys: Box<dyn Core> = match () {
        #[cfg(feature = "ggc")]
        _ if _is_ggc => ggc::GameGirl::with_cart(cart, path, config),
        #[cfg(feature = "gga")]
        _ if _is_gga => gga::GameGirlAdv::with_cart(cart, path, config),
        #[cfg(feature = "nds")]
        _ if _is_nds => nds::Nds::with_cart(cart, path, config),

        #[cfg(feature = "gga")]
        _ => {
            log::error!("Failed to detect cart! Guessing GGA.");
            gga::GameGirlAdv::with_cart(cart, path, config)
        }

        #[cfg(not(feature = "gga"))]
        _ => panic!("Failed to detect cart and no GGA core available!."),
    };

    sys.options().running = true;
    sys.options().rom_loaded = true;
    if common::TRACING {
        sys.options().running = false;
        sys.skip_bootrom();
    }
    sys
}

pub fn dummy_core() -> Box<dyn Core> {
    Box::<Dummy>::default()
}

#[derive(Default)]
struct Dummy {
    options: EmulateOptions,
    config: SystemConfig,
}

impl Core for Dummy {
    fn advance_delta(&mut self, _: f32) {}

    fn produce_frame(&mut self) -> Option<Vec<common::Colour>> {
        None
    }

    fn produce_samples(&mut self, _: &mut [f32]) {}

    fn save_state(&mut self) -> Vec<u8> {
        vec![]
    }

    fn load_state(&mut self, _: &[u8]) {}

    fn advance(&mut self) {}

    fn reset(&mut self) {}

    fn skip_bootrom(&mut self) {}

    fn last_frame(&mut self) -> Option<Vec<common::Colour>> {
        None
    }

    fn options(&mut self) -> &mut common::misc::EmulateOptions {
        &mut self.options
    }

    fn config(&self) -> &SystemConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut SystemConfig {
        &mut self.config
    }

    fn set_button(&mut self, _: common::misc::Button, _: bool) {}

    fn screen_size(&self) -> [usize; 2] {
        [160, 144]
    }

    fn make_save(&self) -> Option<common::components::storage::GameSave> {
        None
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
