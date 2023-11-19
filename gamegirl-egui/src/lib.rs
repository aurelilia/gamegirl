// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(trait_upcasting)]

mod app;
mod debug;
mod gui;
mod input;
mod rewind;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub use app::App;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, Stream, StreamConfig,
};
use eframe::egui::Color32;
use gamegirl::Core;

/// Colour type used by the PPU for display output.
pub type Colour = Color32;

/// Setup audio playback on the default audio device using CPAL.
/// Will automatically poll the gg for audio when needed on a separate thread.
/// *NOT* used for synchronization, since audio samples are requested less than
/// 60 times per second, which would lead to choppy display.
/// Make sure to keep the returned Stream around to prevent the audio playback
/// thread from closing.
pub fn setup_cpal(sys: Arc<Mutex<Box<dyn Core>>>) -> Option<Stream> {
    let sr = {
        let core = sys.lock().unwrap();
        core.wanted_sample_rate()
    };
    let device = cpal::default_host().default_output_device().unwrap();
    let stream = device
        .build_output_stream(
            &StreamConfig {
                channels: 2,
                sample_rate: SampleRate(sr),
                buffer_size: BufferSize::Fixed(sr / 30),
            },
            move |data: &mut [f32], _| {
                let mut core = sys.lock().unwrap();
                core.produce_samples(data)
            },
            move |err| panic!("{err}"),
            Some(Duration::from_secs(1)),
        )
        .ok()?;
    stream.play().ok();
    Some(stream)
}
