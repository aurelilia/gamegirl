#![feature(duration_consts_float)]

mod gui;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleRate, Stream, StreamConfig};
use eframe::egui;
use gamegirl::system::io::apu::SAMPLE_RATE;
use gamegirl::system::GameGirl;
use std::env::args;
use std::fs;
use std::sync::{Arc, Mutex};

fn main() {
    let gg = GameGirl::new(fs::read(args().nth(1).unwrap()).unwrap(), None);
    let gg = Arc::new(Mutex::new(gg));
    let _stream = setup_cpal(gg.clone());
    gui::start(gg);
}

fn setup_cpal(gg: Arc<Mutex<GameGirl>>) -> Stream {
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
                data.copy_from_slice(&samples);
            },
            move |err| panic!("{err}"),
        )
        .unwrap();
    stream.play().unwrap();
    stream
}
