#![feature(duration_consts_float)]
#![feature(exclusive_range_pattern)]
#![feature(mixed_integer_ops)]

pub mod gui;
pub mod numutil;
mod storage;
pub mod system;

use crate::system::io::apu::SAMPLE_RATE;
use crate::system::GameGirl;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleRate, Stream, StreamConfig};
use eframe::egui::Color32;
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

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

    let gg = GameGirl::new();
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
pub fn setup_cpal(gg: Arc<Mutex<GameGirl>>) -> Stream {
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
