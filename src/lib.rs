#![feature(duration_consts_float)]
#![feature(exclusive_range_pattern)]
#![feature(mixed_integer_ops)]

pub mod gui;
pub mod numutil;
pub mod system;

use crate::system::io::apu::SAMPLE_RATE;
use crate::system::GameGirl;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleRate, Stream, StreamConfig};
use eframe::egui::Color32;
use std::sync::{Arc, Mutex};

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

pub type Colour = Color32;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let gg = GameGirl::new();
    let gg = Arc::new(Mutex::new(gg));
    let _stream = setup_cpal(gg.clone());
    gui::start(gg, canvas_id)
}

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
                let samples = {
                    let mut gg = gg.lock().unwrap();
                    gg.produce_samples(data.len())
                };
                if let Some(samples) = samples {
                    data.copy_from_slice(&samples);
                }
            },
            move |err| panic!("{err}"),
        )
        .unwrap();
    stream.play().unwrap();
    stream
}
