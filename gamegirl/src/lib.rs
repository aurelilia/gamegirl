// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! This crate contains common structures shared by all systems.

use std::{
    io,
    io::{Cursor, Read},
    path::PathBuf,
    sync::Arc,
};

pub use common::{self, Core};
use common::{common::options::SystemConfig, components::storage::Storage, Common, Time};
#[cfg(feature = "gga")]
pub use gga;
#[cfg(feature = "ggc")]
pub use ggc;
use glow::Context;
#[cfg(feature = "nds")]
pub use nds;
// #[cfg(feature = "psx")]
// pub use psx;
use thiserror::Error;
use zip::result::ZipError;

#[cfg(all(feature = "dynamic", target_family = "unix"))]
pub mod dynamic;
#[cfg(all(feature = "remote-debugger", target_family = "unix"))]
pub mod remote_debugger;

#[derive(Error, Debug)]
pub enum GamegirlError {
    #[error("ROM is too small")]
    RomTooSmall,
    #[error("Zip error: {0}")]
    ZipError(ZipError),
    #[error("IO error: {0}")]
    IoError(io::Error),
    #[error("Console autodetection failed, make sure you have a valid ROM file")]
    AutodetectFailed,
}

/// Save a game to disk.
pub fn save_game(system: &dyn Core, path: Option<PathBuf>) {
    let save = system.make_save();
    if let Some(save) = save {
        Storage::save(path, save);
    }
}

/// Load a cart. Tries to automatically pick the right system kind.
/// ROM can optionally be compressed
pub fn load_cart_maybe_zip(
    cart: Vec<u8>,
    path: Option<PathBuf>,
    config: &SystemConfig,
    _ogl_ctx: Option<Arc<Context>>,
    _ogl_tex_id: u32,
) -> Result<Box<dyn Core>, GamegirlError> {
    let reader = Cursor::new(&cart);
    let zip = zip::ZipArchive::new(reader);
    match zip {
        Ok(mut archive) => {
            let mut rom = Vec::new();
            archive
                .by_index(0)
                .map_err(GamegirlError::ZipError)?
                .read_to_end(&mut rom)
                .map_err(GamegirlError::IoError)?;
            load_cart(rom, path, config, _ogl_ctx, _ogl_tex_id)
        }
        Err(_) => load_cart(cart, path, config, _ogl_ctx, _ogl_tex_id),
    }
}

/// Load a cart. Tries to automatically pick the right system kind.
pub fn load_cart(
    cart: Vec<u8>,
    path: Option<PathBuf>,
    config: &SystemConfig,
    _ogl_ctx: Option<Arc<Context>>,
    _ogl_tex_id: u32,
) -> Result<Box<dyn Core>, GamegirlError> {
    if cart.len() < 0x120 {
        return Err(GamegirlError::RomTooSmall);
    }

    // We detect GG(C) carts by the first 2 bytes of the "Nintendo" logo header
    // that is present on every cartridge.
    let _is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
    // We detect GGA carts by a zero-filled header region
    let _is_gga = cart.iter().skip(0xB5).take(6).all(|b| *b == 0);
    // We detect NDS carts by a zero-filled header region
    let _is_nds = cart.iter().skip(0x15).take(6).all(|b| *b == 0);
    // We detect iNES files by the header
    let _is_nes = cart[0] == b'N' && cart[1] == b'E' && cart[2] == b'S';
    // We detect PSX games by being ISOs
    // #[cfg(feature = "psx")]
    // use std::os::unix::prelude::OsStrExt;
    // #[cfg(feature = "psx")]
    // let _is_psx = path
    //     .as_ref()
    //     .map(|e| e.extension().unwrap().as_bytes() == b"iso")
    //     .unwrap_or(false);

    let mut sys: Box<dyn Core> = match () {
        #[cfg(feature = "ggc")]
        _ if _is_ggc => ggc::GameGirl::with_cart(cart, path, config),
        #[cfg(feature = "nds")]
        _ if _is_nds => nds::Nds::with_cart(cart, path, config),
        #[cfg(feature = "gga")]
        _ if _is_gga => gga::GameGirlAdv::new(Some(cart), path, config),
        // #[cfg(feature = "psx")]
        // _ if _is_psx => psx::PlayStation::with_iso(cart, path, config, _ogl_ctx, _ogl_tex_id),
        // #[cfg(feature = "nes")]
        // _ if _is_nes => nes::Nes::with_cart(cart, path, config),
        #[cfg(feature = "gga")]
        _ => {
            log::error!("Failed to detect cart! Guessing GGA.");
            gga::GameGirlAdv::new(Some(cart), path, config)
        }

        #[cfg(not(feature = "gga"))]
        _ => return Err(GamegirlError::AutodetectFailed),
    };

    sys.c_mut().debugger.running = config.run_on_open;
    if config.skip_bootrom {
        sys.skip_bootrom();
    }
    Ok(sys)
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
        vec![]
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

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_rom(&self) -> Vec<u8> {
        vec![]
    }

    fn c(&self) -> &Common {
        &self.c
    }

    fn c_mut(&mut self) -> &mut Common {
        &mut self.c
    }
}
