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
#[cfg(feature = "gga")]
pub use gga;
#[cfg(feature = "ggc")]
pub use ggc;
#[cfg(feature = "nds")]
pub use nds;
#[cfg(feature = "psx")]
pub use psx;

#[cfg(feature = "remote-debugger")]
pub mod remote_debugger;

/// Macro for forwarding functions on the main system enum to individual
/// systems.
macro_rules! forward_fn {
    ($name:ident, $ret:ty, $arg:ty) => {
        pub fn $name(&mut self, arg: $arg) -> $ret {
            match self {
                #[cfg(feature = "ggc")]
                System::GGC(gg) => gg.$name(arg),
                #[cfg(feature = "gga")]
                System::GGA(gg) => gg.$name(arg),
                #[cfg(feature = "nds")]
                System::NDS(ds) => ds.$name(arg),
                #[cfg(feature = "psx")]
                System::PSX(_ps) => todo!(),
            }
        }
    };
    ($name:ident, $ret:ty) => {
        pub fn $name(&mut self) -> $ret {
            match self {
                #[cfg(feature = "ggc")]
                System::GGC(gg) => gg.$name(),
                #[cfg(feature = "gga")]
                System::GGA(gg) => gg.$name(),
                #[cfg(feature = "nds")]
                System::NDS(ds) => ds.$name(),
                #[cfg(feature = "psx")]
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
                #[cfg(feature = "ggc")]
                System::GGC($sys) => $expr,
                #[cfg(feature = "gga")]
                System::GGA($sys) => $expr,
                #[cfg(feature = "nds")]
                System::NDS($sys) => $expr,
                #[cfg(feature = "psx")]
                System::PSX($sys) => todo!(),
            }
        }
    };
}

/// Enum for the system currently loaded.
pub enum System {
    /// A GGC. Is also used for DMG games.
    #[cfg(feature = "ggc")]
    GGC(Box<ggc::GameGirl>),
    /// A GGA. Only used for GGA games.
    #[cfg(feature = "gga")]
    GGA(Box<gga::GameGirlAdv>),
    /// An NDS. Only used for NDS games.
    #[cfg(feature = "nds")]
    NDS(Box<nds::Nds>),
    /// A PSX. Only used for PSX games, obviously.
    #[cfg(feature = "psx")]
    PSX(Box<psx::PlayStation>),
}

impl System {
    forward_fn!(advance_delta, (), f32);
    forward_fn!(produce_frame, Option<Vec<Colour>>);
    forward_fn!(produce_samples, (), &mut [f32]);

    #[cfg(feature = "serde")]
    forward_fn!(save_state, Vec<u8>);
    #[cfg(feature = "serde")]
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
            #[cfg(feature = "ggc")]
            System::GGC(gg) => ggc::io::joypad::Joypad::set(gg, btn, pressed),
            #[cfg(feature = "gga")]
            System::GGA(gg) => gg.set_button(btn, pressed),
            _ => todo!(),
        }
    }

    /// Returns the screen size for the current system.
    pub fn screen_size(&self) -> [usize; 2] {
        match self {
            #[cfg(feature = "ggc")]
            System::GGC(_) => [160, 144],
            #[cfg(feature = "gga")]
            System::GGA(_) => [240, 160],
            #[cfg(feature = "nds")]
            System::NDS(_) => [256, 192 * 2],
            #[cfg(feature = "psx")]
            System::PSX(_) => [640, 480],
        }
    }

    /// Save the game to disk.
    pub fn save_game(&self, path: Option<PathBuf>) {
        let save = match self {
            #[cfg(feature = "ggc")]
            System::GGC(gg) => gg.cart.make_save(),
            #[cfg(feature = "gga")]
            System::GGA(gg) => gg.cart.make_save(),
            _ => todo!(),
        };
        if let Some(save) = save {
            Storage::save(path, save);
        }
    }

    #[cfg(feature = "ggc")]
    pub fn as_ggc(&self) -> &ggc::GameGirl {
        match self {
            System::GGC(gg) => gg,
            _ => panic!(),
        }
    }

    #[cfg(feature = "gga")]
    pub fn as_gga(&self) -> &gga::GameGirlAdv {
        match self {
            System::GGA(gg) => gg,
            _ => panic!(),
        }
    }

    #[cfg(feature = "gga")]
    pub fn gga_mut(&mut self) -> &mut gga::GameGirlAdv {
        match self {
            System::GGA(gg) => gg,
            _ => panic!(),
        }
    }

    /// Load a cart. Automatically picks the right system kind.
    pub fn load_cart(&mut self, cart: Vec<u8>, path: Option<PathBuf>, config: &SystemConfig) {
        // We detect GG(C) carts by the first 2 bytes of the "Nintendo" logo header
        // that is present on every cartridge.
        let _is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
        // We detect GGA carts by a zero-filled header region
        let _is_gga = cart.iter().skip(0xB5).take(6).all(|b| *b == 0);
        // We detect NDS carts by a zero-filled header region
        let _is_nds = cart.iter().skip(0x15).take(6).all(|b| *b == 0);

        let frame_finished = mem::replace(
            &mut self.options().frame_finished,
            EmulateOptions::serde_frame_finished(),
        );
        match () {
            #[cfg(feature = "ggc")]
            _ if _is_ggc => *self = System::GGC(ggc::GameGirl::with_cart(cart, path, config)),
            #[cfg(feature = "gga")]
            _ if _is_gga => *self = System::GGA(gga::GameGirlAdv::with_cart(cart, path, config)),
            #[cfg(feature = "nds")]
            _ if _is_nds => *self = System::NDS(nds::Nds::with_cart(cart, path, config)),

            #[cfg(feature = "gga")]
            _ => {
                log::error!("Failed to detect cart! Guessing GGA.");
                *self = System::GGA(gga::GameGirlAdv::with_cart(cart, path, config));
            }

            #[cfg(not(feature = "gga"))]
            _ => panic!("Failed to detect cart and no GGA core available!."),
        }

        self.options().frame_finished = frame_finished;
        self.options().running = true;
        self.options().rom_loaded = true;
        if common::TRACING {
            self.options().running = false;
            self.skip_bootrom();
        }
    }
}

impl Default for System {
    fn default() -> Self {
        // We start with a GGA, will be changed later if user loads a GGA cart.
        Self::GGC(Box::default())
    }
}
