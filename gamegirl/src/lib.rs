// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! This crate contains common structures shared by all systems.

use std::{mem, path::PathBuf};

pub use common;
use common::{
    components::storage::Storage,
    misc::{Button, EmulateOptions, SystemConfig},
    Colour,
};
pub use gga;
use gga::GameGirlAdv;
pub use ggc;
use ggc::{
    io::{cartridge::Cartridge, joypad::Joypad},
    GameGirl,
};
pub use nds;
use nds::Nds;
pub use psx;
use psx::PlayStation;

#[cfg(not(target_arch = "wasm32"))]
pub mod remote_debugger;

/// Macro for forwarding functions on the main system enum to individual
/// systems.
macro_rules! forward_fn {
    ($name:ident, $ret:ty, $arg:ty) => {
        pub fn $name(&mut self, arg: $arg) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(arg),
                System::GGA(gg) => gg.$name(arg),
                System::NDS(ds) => ds.$name(arg),
                System::PSX(_ps) => todo!(),
            }
        }
    };
    ($name:ident, $ret:ty) => {
        pub fn $name(&mut self) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(),
                System::GGA(gg) => gg.$name(),
                System::NDS(ds) => ds.$name(),
                System::PSX(ps) => ps.$name(),
            }
        }
    };
    ($name:ident) => {
        forward_fn!($name, ());
    };
}

/// Macro for forwarding properties on the main system enum to individual
/// systems.
macro_rules! forward_member {
    ($self:ty, $name:ident, $ret:ty, $sys:ident, $expr:expr) => {
        pub fn $name(self: $self) -> $ret {
            match self {
                System::GGC($sys) => $expr,
                System::GGA($sys) => $expr,
                System::NDS($sys) => $expr,
                System::PSX($sys) => todo!(),
            }
        }
    };
}

/// Enum for the system currently loaded.
pub enum System {
    /// A GGC. Is also used for DMG games.
    GGC(Box<GameGirl>),
    /// A GGA. Only used for GGA games.
    GGA(Box<GameGirlAdv>),
    /// An NDS. Only used for NDS games.
    NDS(Box<Nds>),
    /// A PSX. Only used for PSX games, obviously.
    PSX(Box<PlayStation>),
}

impl System {
    forward_fn!(advance_delta, (), f32);
    forward_fn!(produce_frame, Option<Vec<Colour>>);
    forward_fn!(produce_samples, (), &mut [f32]);
    forward_fn!(save_state, Vec<u8>);
    forward_fn!(load_state, (), &[u8]);

    forward_fn!(advance);
    forward_fn!(reset);
    forward_fn!(skip_bootrom);

    forward_member!(
        &mut Self,
        last_frame,
        Option<Vec<Colour>>,
        _sys,
        _sys.ppu.last_frame.take()
    );
    forward_member!(
        &mut Self,
        options,
        &mut EmulateOptions,
        _sys,
        &mut _sys.options
    );
    forward_member!(&Self, config, &SystemConfig, _sys, &_sys.config);
    forward_member!(
        &mut Self,
        config_mut,
        &mut SystemConfig,
        _sys,
        &mut _sys.config
    );

    /// Set a button on the joypad.
    pub fn set_button(&mut self, btn: Button, pressed: bool) {
        match self {
            System::GGC(gg) => Joypad::set(gg, btn, pressed),
            System::GGA(gg) => gg.set_button(btn, pressed),
            _ => todo!(),
        }
    }

    /// Returns the screen size for the current system.
    pub fn screen_size(&self) -> [usize; 2] {
        match self {
            System::GGC(_) => [160, 144],
            System::GGA(_) => [240, 160],
            System::NDS(_) => [256, 192 * 2],
            System::PSX(_) => [640, 480],
        }
    }

    /// Save the game to disk.
    pub fn save_game(&self, path: Option<PathBuf>) {
        let save = match self {
            System::GGC(gg) => gg.cart.make_save(),
            System::GGA(gg) => gg.cart.make_save(),
            _ => todo!(),
        };
        if let Some(save) = save {
            Storage::save(path, save);
        }
    }

    pub fn as_ggc(&self) -> &GameGirl {
        match self {
            System::GGC(gg) => gg,
            _ => panic!(),
        }
    }

    pub fn as_gga(&self) -> &GameGirlAdv {
        match self {
            System::GGA(gg) => gg,
            _ => panic!(),
        }
    }

    pub fn gga_mut(&mut self) -> &mut GameGirlAdv {
        match self {
            System::GGA(gg) => gg,
            _ => panic!(),
        }
    }

    /// Load a cart. Automatically picks the right system kind.
    pub fn load_cart(&mut self, cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) {
        // We detect GG(C) carts by the first 2 bytes of the "Nintendo" logo header
        // that is present on every cartridge.
        let is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
        // We detect GGA carts by a zero-filled header region
        let is_gga = cart.iter().skip(0xB5).take(6).all(|b| *b == 0);
        // We detect NDS carts by a zero-filled header region
        let is_nds = cart.iter().skip(0x15).take(6).all(|b| *b == 0);

        let frame_finished = mem::replace(
            &mut self.options().frame_finished,
            EmulateOptions::serde_frame_finished(),
        );
        match () {
            _ if is_ggc => self.load_ggc(cart, path, config),
            _ if is_gga => *self = System::GGA(GameGirlAdv::with_cart(cart, path, config)),
            _ if is_nds => self.load_nds(cart, path, config),
            _ => {
                log::error!("Failed to detect cart! Guessing GGA.");
                *self = System::GGA(GameGirlAdv::with_cart(cart, path, config));
            }
        }

        self.options().frame_finished = frame_finished;
        self.options().running = true;
        self.options().rom_loaded = true;
        if common::TRACING {
            self.options().running = false;
            self.skip_bootrom();
        }
    }

    fn load_ggc(&mut self, cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) {
        let mut cart = Cartridge::from_rom(cart);
        if let Some(save) = Storage::load(path, cart.title(true)) {
            cart.load_save(save);
        }

        let mut ggc = Box::new(GameGirl::default());
        ggc.load_cart(cart, config, false);
        ggc.options.frame_finished = mem::replace(
            &mut self.options().frame_finished,
            EmulateOptions::serde_frame_finished(),
        );
        *self = Self::GGC(ggc);
    }

    fn load_nds(&mut self, cart: Vec<u8>, _path: Option<PathBuf>, config: &SystemConfig) {
        let mut nds = Box::new(Nds::default());
        nds.config = config.clone();
        nds.cart.load_rom(cart);
        nds.init_memory();
        nds.options.frame_finished = mem::replace(
            &mut self.options().frame_finished,
            EmulateOptions::serde_frame_finished(),
        );

        *self = Self::NDS(nds);
    }
}

impl Default for System {
    fn default() -> Self {
        // We start with a GGC, will be changed later if user loads a GGA cart.
        Self::GGC(Box::default())
    }
}
