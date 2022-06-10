//! This file contains common structures shared by GGC and GGA.

use crate::ggc::io::cartridge::Cartridge;
use crate::ggc::io::joypad::Joypad;
use crate::ggc::GGOptions;
use crate::storage::Storage;
use crate::{ggc::GameGirl, Colour};
use serde::{Deserialize, Serialize};
use std::mem;
use std::path::PathBuf;
use crate::gga::GameGirlAdv;

/// Macro for forwarding functions on the main system enum to individual systems.
macro_rules! forward {
    ($name:ident, $ret:ty, $arg:ty) => {
        pub fn $name(&mut self, arg: $arg) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(arg),
                System::GGA(_gg) => todo!(),
            }
        }
    };
    ($name:ident, $ret:ty) => {
        pub fn $name(&mut self) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(),
                System::GGA(_gg) => todo!(),
            }
        }
    };
    ($name:ident) => {
        forward!($name, ());
    };
}

/// Enum for the system currently loaded.
pub enum System {
    /// A GGC. Is also used for DMG games.
    GGC(GameGirl),
    /// A GGA. Only used for GGA games.
    GGA(GameGirlAdv),
}

impl System {
    forward!(advance_delta, (), f32);
    forward!(produce_frame, Option<Vec<Colour>>);
    forward!(produce_samples, (), &mut [f32]);
    forward!(advance);
    forward!(reset);
    forward!(save_state, Vec<u8>);
    forward!(load_state, (), &[u8]);

    /// Set a button on the joypad.
    pub fn set_button(&mut self, btn: Button, pressed: bool) {
        match self {
            System::GGC(gg) => Joypad::set(gg, btn, pressed),
            System::GGA(_gg) => todo!(),
        }
    }

    /// Get the last frame produced by the PPU.
    pub fn last_frame(&mut self) -> Option<Vec<Colour>> {
        match self {
            System::GGC(gg) => gg.mmu.ppu.last_frame.take(),
            System::GGA(_gg) => todo!(),
        }
    }

    /// Get emulation options.
    pub fn options(&mut self) -> &mut EmulateOptions {
        match self {
            System::GGC(gg) => &mut gg.options,
            System::GGA(_gg) => todo!(),
        }
    }

    /// Get emulation config.
    pub fn config(&mut self) -> &mut GGOptions {
        match self {
            System::GGC(gg) => &mut gg.config,
            System::GGA(_gg) => todo!(),
        }
    }

    /// Returns the screen size for the current system.
    pub fn screen_size(&self) -> [usize; 2] {
        match self {
            System::GGC(_) => [160, 144],
            System::GGA(_) => [240, 160],
        }
    }

    /// Save the game to disk.
    pub fn save_game(&self, path: Option<PathBuf>) {
        match self {
            System::GGC(gg) => {
                if let Some(save) = gg.mmu.cart.make_save() {
                    Storage::save(path, save);
                }
            }
            System::GGA(_) => todo!(),
        }
    }

    /// Create a new system.
    pub fn new() -> Self {
        // We start with a GGC, will be changed later if user loads a GGA cart.
        Self::GGC(GameGirl::new())
    }

    /// Load a cart.
    pub fn load_cart(&mut self, cart: Vec<u8>, path: Option<PathBuf>, config: &GGOptions) {
        let is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
        if is_ggc {
            let mut cart = Cartridge::from_rom(cart);
            if let Some(save) = Storage::load(path, cart.title(true)) {
                cart.load_save(save);
            }

            let mut ggc = GameGirl::new();
            ggc.load_cart(cart, &config, false);
            ggc.options.frame_finished = mem::replace(
                &mut self.options().frame_finished,
                EmulateOptions::serde_frame_finished(),
            );
            *self = Self::GGC(ggc);
        } else {
            todo!()
        }
    }
}

/// Options that are used by the GUI and shared between GGC/GGA.
#[derive(Deserialize, Serialize)]
pub struct EmulateOptions {
    /// If the system is running. If false, any calls to [advance_delta] and [produce_samples] do nothing.
    pub running: bool,
    /// If there is a ROM loaded / cartridge inserted.
    pub rom_loaded: bool,
    /// If the audio samples produced by [produce_samples] should be in reversed order.
    /// `true` while rewinding.
    pub invert_audio_samples: bool,
    /// Speed multiplier the system should run at.
    /// ex. 1x is regular speed, 2x is double speed.
    /// Affects [advance_delta] and sound sample output.
    pub speed_multiplier: usize,
    /// Called when a frame is finished rendering. (End of VBlank)
    #[serde(skip)]
    #[serde(default = "EmulateOptions::serde_frame_finished")]
    pub frame_finished: Box<dyn Fn(&GameGirl) + Send>,
}

impl EmulateOptions {
    pub fn new() -> Self {
        Self {
            running: false,
            rom_loaded: false,
            invert_audio_samples: false,
            speed_multiplier: 1,
            frame_finished: Box::new(|_| ()),
        }
    }

    fn serde_frame_finished() -> Box<dyn Fn(&GameGirl) + Send> {
        Box::new(|_| ())
    }
}

/// Buttons on a system. For GGC, L/R are unused.
#[derive(Debug, Copy, Clone, PartialEq, Hash, Deserialize, Serialize)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
    L,
    R,
}

impl Button {
    pub const BUTTONS: [Self; 10] = [
        Self::A,
        Self::B,
        Self::Select,
        Self::Start,
        Self::Right,
        Self::Left,
        Self::Up,
        Self::Down,
        Self::L,
        Self::R,
    ];
}
