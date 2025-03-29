use core::time::Duration;
use std::{
    boxed::Box,
    sync::{Arc, Mutex},
};

use common::Core;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, Stream, StreamConfig,
};

pub struct AudioStream(#[allow(dead_code)] Option<Stream>);

impl AudioStream {
    pub fn empty() -> Self {
        Self(None)
    }
}

/// Setup audio playback on the default audio device using CPAL.
/// Will automatically poll the gg for audio when needed on a separate thread.
/// *NOT* used for synchronization, since audio samples are requested less than
/// 60 times per second, which would lead to choppy display.
/// Make sure to keep the returned Stream around to prevent the audio playback
/// thread from closing.
pub fn setup(sys: Arc<Mutex<Box<dyn Core>>>) -> AudioStream {
    let sr = {
        let core = sys.lock().unwrap();
        core.c().config.sample_rate as u32
    };
    let device = cpal::default_host().default_output_device().unwrap();
    if let Some(stream) = device
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
        .ok()
    {
        stream.play().ok();
        AudioStream(Some(stream))
    } else {
        AudioStream(None)
    }
}
