use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
};

use gamegirl::{
    SystemConfig,
    frontend::{filter::Blend, input::Input, rewinder::RewinderConfig},
};

use crate::gui::input::GtkKey;

/// User-configurable options.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub sys: SystemConfig,
    /// Input configuration.
    pub input: Input<GtkKey>,
    /// Rewinder configuration.
    pub rewind: RewinderConfig,
    /// Texture filter to use.
    pub texture_filter: TextureFilter,
    /// Blending filter to use.
    pub blend_filter: Blend,
    /// If aspect ration should be preserved.
    pub preserve_aspect_ratio: bool,
}

impl Options {
    pub fn empty() -> Self {
        Self {
            sys: Default::default(),
            input: Input::new(),
            rewind: RewinderConfig::default(),
            texture_filter: TextureFilter::Nearest,
            blend_filter: Blend::None,
            preserve_aspect_ratio: true,
        }
    }

    pub fn from_disk() -> Self {
        let path = dirs::config_dir().unwrap().join("gamegirl/config.bin");
        let data = File::open(&path).ok().and_then(|file| {
            bincode::serde::decode_from_reader(&mut BufReader::new(file), bincode::config::legacy())
                .ok()
        });
        data.unwrap_or_else(|| Self::empty())
    }

    pub fn to_disk(&self) {
        let path = dirs::config_dir().unwrap().join("gamegirl/config.bin");
        fs::create_dir(&path.parent().unwrap()).ok();
        File::create(path).ok().and_then(|file| {
            bincode::serde::encode_into_std_write(
                self,
                &mut BufWriter::new(file),
                bincode::config::legacy(),
            )
            .ok()
        });
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::from_disk()
    }
}

#[derive(Copy, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TextureFilter {
    Nearest,
    Linear,
    Trilinear,
}
