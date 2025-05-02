// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! This crate contains common structures shared by all systems.

#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use alloc::{boxed::Box, vec::Vec};

pub use common::{self, common::options::*, components::storage::*, Core};
use common::{Common, Time};
#[cfg(feature = "gga")]
pub use gga;
#[cfg(feature = "ggc")]
pub use ggc;
#[cfg(feature = "nds")]
pub use nds;
// #[cfg(feature = "psx")]
// pub use psx;
use thiserror::Error;

#[cfg(all(feature = "dynamic", target_family = "unix"))]
pub mod dynamic;
#[cfg(feature = "frontend")]
pub mod frontend;
#[cfg(all(feature = "remote-debugger", target_family = "unix"))]
pub mod remote_debugger;

#[derive(Error, Debug)]
pub enum GamegirlError {
    #[error("ROM is too small")]
    RomTooSmall,

    #[cfg(feature = "std")]
    #[error("Zip error: {0}")]
    ZipError(zip::result::ZipError),

    #[cfg(feature = "std")]
    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("Console autodetection failed, make sure you have a valid ROM file")]
    AutodetectFailed,
}

/// Load a cart. Tries to automatically pick the right system kind.
/// ROM can optionally be compressed
#[cfg(feature = "std")]
pub fn load_cart_maybe_zip(
    mut cart: GameCart,
    config: &SystemConfig,
) -> Result<Box<dyn Core>, GamegirlError> {
    use std::io::{Cursor, Read};

    let reader = Cursor::new(&cart.rom);
    let zip = zip::ZipArchive::new(reader);
    match zip {
        Ok(mut archive) => {
            let mut rom = Vec::new();
            archive
                .by_index(0)
                .map_err(GamegirlError::ZipError)?
                .read_to_end(&mut rom)
                .map_err(GamegirlError::IoError)?;
            cart.rom = rom;
            load_cart(cart, config)
        }
        Err(_) => load_cart(cart, config),
    }
}

/// Load a cart. Tries to automatically pick the right system kind.
pub fn load_cart(cart: GameCart, config: &SystemConfig) -> Result<Box<dyn Core>, GamegirlError> {
    let mut sys = load_inner(cart, config)?;
    sys.c_mut().debugger.running = config.run_on_open;
    if config.skip_bootrom {
        sys.skip_bootrom();
    }
    Ok(sys)
}

fn load_inner(cart: GameCart, config: &SystemConfig) -> Result<Box<dyn Core>, GamegirlError> {
    if cart.rom.len() < 0x120 {
        return Err(GamegirlError::RomTooSmall);
    }
    let mut cart = Some(cart);

    #[cfg(feature = "nds")]
    if let Some(core) = nds::Nds::try_new(&mut cart, config) {
        return Ok(core);
    }
    #[cfg(feature = "ggc")]
    if let Some(core) = ggc::GameGirl::try_new(&mut cart, config) {
        return Ok(core);
    }
    #[cfg(feature = "gga")]
    if let Some(core) = gga::GameGirlAdv::try_new(&mut cart, config) {
        return Ok(core);
    }

    Err(GamegirlError::AutodetectFailed)
}

pub fn dummy_core() -> Box<dyn Core> {
    Box::<Dummy>::default()
}

#[derive(Default)]
struct Dummy {
    c: Common,
}

impl Core for Dummy {
    fn advance_delta(&mut self, _: f32) {}

    fn save_state(&mut self) -> Vec<u8> {
        Vec::new()
    }

    fn load_state(&mut self, _: &[u8]) {}

    fn advance(&mut self) {}

    fn reset(&mut self) {}

    fn skip_bootrom(&mut self) {}

    fn get_time(&self) -> Time {
        0
    }

    fn screen_size(&self) -> [usize; 2] {
        [160, 144]
    }

    fn make_save(&self) -> Option<common::components::storage::GameSave> {
        None
    }

    fn get_rom(&self) -> Vec<u8> {
        Vec::new()
    }

    fn c(&self) -> &Common {
        &self.c
    }

    fn c_mut(&mut self) -> &mut Common {
        &mut self.c
    }

    fn try_new(
        _cart: &mut Option<common::components::storage::GameCart>,
        config: &SystemConfig,
    ) -> Option<Box<Self>> {
        Some(Box::new(Self {
            c: Common::with_config(config.clone()),
        }))
    }
}
