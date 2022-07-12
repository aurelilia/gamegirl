// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

//! This file contains common structures shared by GGC and GGA.

use std::{iter, mem, path::PathBuf};

use elf_rs::{Elf, ElfFile};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    gga::{cpu::Cpu, GameGirlAdv},
    ggc::{
        io::{cartridge::Cartridge, joypad::Joypad},
        GameGirl,
    },
    psx::PlayStation,
    storage::Storage,
    Colour,
};

/// Audio sample rate of all emulated systems.
pub const SAMPLE_RATE: u32 = 44100;

/// Macro for forwarding functions on the main system enum to individual
/// systems.
macro_rules! forward {
    ($name:ident, $ret:ty, $arg:ty) => {
        pub fn $name(&mut self, arg: $arg) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(arg),
                System::GGA(gg) => gg.$name(arg),
                System::PSX(_ps) => todo!(),
            }
        }
    };
    ($name:ident, $ret:ty) => {
        pub fn $name(&mut self) -> $ret {
            match self {
                System::GGC(gg) => gg.$name(),
                System::GGA(gg) => gg.$name(),
                System::PSX(ps) => ps.$name(),
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
    GGC(Box<GameGirl>),
    /// A GGA. Only used for GGA games.
    GGA(Box<GameGirlAdv>),
    /// A PSX. Only used for PSX games, obviously.
    PSX(Box<PlayStation>),
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
    forward!(skip_bootrom);

    /// Set a button on the joypad.
    pub fn set_button(&mut self, btn: Button, pressed: bool) {
        match self {
            System::GGC(gg) => Joypad::set(gg, btn, pressed),
            System::GGA(gg) => gg.set_button(btn, pressed),
            System::PSX(_ps) => todo!(),
        }
    }

    /// Get the last frame produced by the PPU.
    pub fn last_frame(&mut self) -> Option<Vec<Colour>> {
        match self {
            System::GGC(gg) => gg.ppu.last_frame.take(),
            System::GGA(gg) => gg.ppu.last_frame.take(),
            System::PSX(_ps) => todo!(),
        }
    }

    /// Get emulation options.
    pub fn options(&mut self) -> &mut EmulateOptions {
        match self {
            System::GGC(gg) => &mut gg.options,
            System::GGA(gg) => &mut gg.options,
            System::PSX(ps) => &mut ps.options,
        }
    }

    /// Get emulation config.
    pub fn config(&self) -> &SystemConfig {
        match self {
            System::GGC(gg) => &gg.config,
            System::GGA(gg) => &gg.config,
            System::PSX(ps) => &ps.config,
        }
    }

    /// Get emulation config.
    pub fn config_mut(&mut self) -> &mut SystemConfig {
        match self {
            System::GGC(gg) => &mut gg.config,
            System::GGA(gg) => &mut gg.config,
            System::PSX(ps) => &mut ps.config,
        }
    }

    /// Returns the screen size for the current system.
    pub fn screen_size(&self) -> [usize; 2] {
        match self {
            System::GGC(_) => [160, 144],
            System::GGA(_) => [240, 160],
            System::PSX(_) => [640, 480],
        }
    }

    /// Save the game to disk.
    pub fn save_game(&self, path: Option<PathBuf>) {
        match self {
            System::GGC(gg) => {
                if let Some(save) = gg.cart.make_save() {
                    Storage::save(path, save);
                }
            }
            System::GGA(gg) => {
                if let Some(save) = gg.cart.make_save() {
                    Storage::save(path, save);
                }
            }
            System::PSX(_ps) => todo!(),
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

        if is_ggc {
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
        } else {
            let (mut cart, elf) = if let Some(elf_read) = Self::decode_elf(&cart) {
                (elf_read, true)
            } else {
                (cart, false)
            };

            // Paging implementation requires this to prevent reading unallocated memory
            let until_full_page = 0x7FFF - (cart.len() & 0x7FFF);
            cart.extend(iter::repeat(0).take(until_full_page));

            let mut gga = Box::new(GameGirlAdv::default());
            gga.config = config.clone();
            gga.cart.load_rom(cart);
            if let Some(save) = Storage::load(path, gga.cart.title()) {
                gga.cart.load_save(save);
            }
            gga.init_memory();
            gga.options.frame_finished = mem::replace(
                &mut self.options().frame_finished,
                EmulateOptions::serde_frame_finished(),
            );

            if elf {
                gga.skip_bootrom();
            }

            // Fill the prefetch
            Cpu::pipeline_stall(&mut gga);
            *self = Self::GGA(gga);
        }

        self.options().running = true;
        self.options().rom_loaded = true;
        if crate::TRACING {
            self.options().running = false;
            self.skip_bootrom();
        }
    }

    fn decode_elf(cart: &[u8]) -> Option<Vec<u8>> {
        let elf = Elf::from_bytes(cart).ok()?;
        let mut buf = vec![0; 0x1FF_FFFF];

        for header in elf
            .section_header_iter()
            .filter(|h| (0x800_0000..=0x9FF_FFFF).contains(&h.addr()))
        {
            let dst_offs = header.addr() - 0x800_0000;
            for (src, dst) in header
                .content()
                .iter()
                .zip(buf.iter_mut().skip(dst_offs as usize))
            {
                *dst = *src;
            }
        }

        Some(buf)
    }
}

impl Default for System {
    fn default() -> Self {
        // We start with a GGC, will be changed later if user loads a GGA cart.
        Self::GGC(Box::default())
    }
}

/// Options that are used by the GUI and shared between GGC/GGA.
/// These can be changed at runtime.
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

/// Configuration used when initializing the system.
/// These options don't change at runtime.
#[derive(Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// How to handle CGB mode.
    pub mode: CgbMode,
    /// If save states should be compressed.
    pub compress_savestates: bool,
    /// If CGB colours should be corrected.
    pub cgb_colour_correction: bool,
    /// Audio volume multiplier
    pub volume: f32,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            mode: CgbMode::Prefer,
            compress_savestates: false,
            cgb_colour_correction: false,
            volume: 0.5,
        }
    }
}

/// How to handle CGB mode depending on cart compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CgbMode {
    /// Always run in CGB mode, even when the cart does not support it.
    /// If it does not, it is run in DMG compatibility mode, just like on a
    /// real CGB.
    Always,
    /// If the cart has CGB support, run it as CGB; if not, don't.
    Prefer,
    /// Never run the cart in CGB mode unless it requires it.
    Never,
}

/// Borrowed system enum used for "end of frame" callbacks on all cores.
/// These are mainly used for rewinding savestates.
pub enum BorrowedSystem<'s> {
    GGC(&'s GameGirl),
    GGA(&'s GameGirlAdv),
}

/// Buttons on a system. For GGC, L/R are unused.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[repr(C)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
    R,
    L,
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
        Self::R,
        Self::L,
    ];
}

/// Serialize an object that can be loaded with [deserialize].
/// It is (optionally zstd-compressed) bincode.
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
/// It is (optionally zstd-compressed) bincode.
pub fn deserialize<T: DeserializeOwned>(state: &[u8], with_zstd: bool) -> T {
    if with_zstd {
        let decoder = zstd::stream::Decoder::new(state).unwrap();
        bincode::deserialize_from(decoder).unwrap()
    } else {
        bincode::deserialize(state).unwrap()
    }
}
