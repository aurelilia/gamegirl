//! This file contains common structures shared by GGC and GGA.

use crate::{
    gga::{cpu::Cpu, GameGirlAdv},
    ggc::{
        io::{cartridge::Cartridge, joypad::Joypad},
        GGConfig, GameGirl,
    },
    storage::Storage,
    Colour,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{mem, path::PathBuf};

/// Macro for forwarding functions on the main system enum to individual
/// systems.
macro_rules! forward {
    ($name:ident, $ret:ty, $arg:ty) => {
        pub fn $name(&mut self, arg: $arg) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(arg),
                System::GGA(gg) => gg.$name(arg),
            }
        }
    };
    ($name:ident, $ret:ty) => {
        pub fn $name(&mut self) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(),
                System::GGA(gg) => gg.$name(),
            }
        }
    };
    ($name:ident) => {
        forward!($name, ());
    };
}

/// Enum for the system currently loaded.
#[derive(Deserialize, Serialize)]
pub enum System {
    /// A GGC. Is also used for DMG games.
    GGC(GameGirl),
    /// A GGA. Only used for GGA games.
    GGA(Box<GameGirlAdv>),
}

impl System {
    // TODO These 5 functions are heavily duplicated, not nice.
    forward!(advance_delta, (), f32);
    forward!(produce_frame, Option<Vec<Colour>>);
    forward!(produce_samples, (), &mut [f32]);
    forward!(save_state, Vec<u8>);
    forward!(load_state, (), &[u8]);

    forward!(advance);
    forward!(reset);

    /// Set a button on the joypad.
    pub fn set_button(&mut self, btn: Button, pressed: bool) {
        match self {
            System::GGC(gg) => Joypad::set(gg, btn, pressed),
            System::GGA(_gg) => (),
        }
    }

    /// Get the last frame produced by the PPU.
    pub fn last_frame(&mut self) -> Option<Vec<Colour>> {
        match self {
            System::GGC(gg) => gg.mmu.ppu.last_frame.take(),
            System::GGA(gg) => gg.ppu.last_frame.take(),
        }
    }

    /// Get emulation options.
    pub fn options(&mut self) -> &mut EmulateOptions {
        match self {
            System::GGC(gg) => &mut gg.options,
            System::GGA(gg) => &mut gg.options,
        }
    }

    /// Get emulation config.
    pub fn config(&self) -> &GGConfig {
        match self {
            System::GGC(gg) => &gg.config,
            System::GGA(gg) => &gg.config,
        }
    }

    /// Get emulation config.
    pub fn config_mut(&mut self) -> &mut GGConfig {
        match self {
            System::GGC(gg) => &mut gg.config,
            System::GGA(gg) => &mut gg.config,
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
            System::GGA(_) => (), // TODO
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

    /// Load a cart.
    pub fn load_cart(&mut self, cart: Vec<u8>, path: Option<PathBuf>, config: &GGConfig) {
        let is_ggc = cart[0x0104] == 0xCE && cart[0x0105] == 0xED;
        if is_ggc {
            let mut cart = Cartridge::from_rom(cart);
            if let Some(save) = Storage::load(path, cart.title(true)) {
                cart.load_save(save);
            }

            let mut ggc = GameGirl::default();
            ggc.load_cart(cart, config, false);
            ggc.options.frame_finished = mem::replace(
                &mut self.options().frame_finished,
                EmulateOptions::serde_frame_finished(),
            );
            *self = Self::GGC(ggc);
        } else {
            let mut gga = Box::new(GameGirlAdv::default());
            gga.cart.rom = cart;
            gga.options.frame_finished = mem::replace(
                &mut self.options().frame_finished,
                EmulateOptions::serde_frame_finished(),
            );
            // Fill the prefetch
            Cpu::fix_prefetch(&mut gga);
            *self = Self::GGA(gga);
        }

        self.options().running = true;
        self.options().rom_loaded = true;
    }
}

impl Default for System {
    fn default() -> Self {
        // We start with a GGC, will be changed later if user loads a GGA cart.
        Self::GGC(GameGirl::default())
    }
}

/// Options that are used by the GUI and shared between GGC/GGA.
#[derive(Deserialize, Serialize)]
pub struct EmulateOptions {
    /// If the system is running. If false, any calls to [advance_delta] and
    /// [produce_samples] do nothing.
    pub running: bool,
    /// If there is a ROM loaded / cartridge inserted.
    pub rom_loaded: bool,
    /// If the audio samples produced by [produce_samples] should be in reversed
    /// order. `true` while rewinding.
    pub invert_audio_samples: bool,
    /// Speed multiplier the system should run at.
    /// ex. 1x is regular speed, 2x is double speed.
    /// Affects [advance_delta] and sound sample output.
    pub speed_multiplier: usize,
    /// Called when a frame is finished rendering. (End of VBlank)
    #[serde(skip)]
    #[serde(default = "EmulateOptions::serde_frame_finished")]
    pub frame_finished: Box<dyn Fn(BorrowedSystem) + Send>,
}

impl EmulateOptions {
    fn serde_frame_finished() -> Box<dyn Fn(BorrowedSystem) + Send> {
        Box::new(|_| ())
    }
}

impl Default for EmulateOptions {
    fn default() -> Self {
        Self {
            running: false,
            rom_loaded: false,
            invert_audio_samples: false,
            speed_multiplier: 1,
            frame_finished: Box::new(|_| ()),
        }
    }
}

pub enum BorrowedSystem<'s> {
    GGC(&'s GameGirl),
    GGA(&'s GameGirlAdv),
}

/// Buttons on a system. For GGC, L/R are unused.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
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

/// Serialize an object that can be loaded with [deserialize].
/// It is optionally zstd-compressed bincode.
pub fn serialize<T: Serialize>(thing: &T, with_zstd: bool) -> Vec<u8> {
    if cfg!(target_arch = "wasm32") {
        // Currently crashes when loading...
        return vec![];
    }
    if with_zstd {
        let mut dest = vec![];
        let mut writer = zstd::stream::Encoder::new(&mut dest, 3).unwrap();
        bincode::serialize_into(&mut writer, thing).unwrap();
        writer.finish().unwrap();
        dest
    } else {
        bincode::serialize(thing).unwrap()
    }
}

/// Deserialize an object that was made with [serialize].
/// It is optionally zstd-compressed bincode.
pub fn deserialize<T: DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    if with_zstd {
        let decoder = zstd::stream::Decoder::new(state).unwrap();
        bincode::deserialize_from(decoder).unwrap()
    } else {
        bincode::deserialize(state).unwrap()
    }
}
