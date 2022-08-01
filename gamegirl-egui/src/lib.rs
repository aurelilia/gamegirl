// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(duration_consts_float)]

pub mod gui;

use std::sync::{Arc, Mutex};

use common::SAMPLE_RATE;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, Stream, StreamConfig,
};
use eframe::egui::Color32;
#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};
use gamegirl::System;

/// Colour type used by the PPU for display output.
pub type Colour = Color32;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct Handle(Stream);

/// Start the emulator on WASM. See web/index.html for usage.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<Handle, eframe::wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let gg = System::default();
    let gg = Arc::new(Mutex::new(gg));
    let stream = setup_cpal(gg.clone());
    gui::start(gg, canvas_id).map(|_| Handle(stream))
}

/// Setup audio playback on the default audio device using CPAL.
/// Will automatically poll the gg for audio when needed on a separate thread.
/// *NOT* used for synchronization, since audio samples are requested less than
/// 60 times per second, which would lead to choppy display.
/// Make sure to keep the returned Stream around to prevent the audio playback
/// thread from closing.
pub fn setup_cpal(gg: Arc<Mutex<System>>) -> Stream {
    let device = cpal::default_host().default_output_device().unwrap();
    let stream = device
        .build_output_stream(
            &StreamConfig {
                channels: 2,
                sample_rate: SampleRate(SAMPLE_RATE),
                buffer_size: BufferSize::Default,
            },
            move |data: &mut [f32], _| {
                let mut gg = gg.lock().unwrap();
                gg.produce_samples(data)
            },
            move |err| panic!("{err}"),
        )
        .unwrap();
    stream.play().unwrap();
    stream
}
