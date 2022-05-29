use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, SampleRate, Stream, StreamConfig, SupportedOutputConfigs,
    SupportedStreamConfigRange,
};
use eframe::epaint::{ColorImage, ImageDelta, TextureId};
use gamegirl::system::io::apu::SAMPLE_RATE;
use std::sync::{Arc, Mutex};

use crate::egui::{Color32, Event, ImageData};
use crate::{egui, GameGirl};
use gamegirl::system::io::joypad::{Button, Joypad};

pub type Colour = Color32;

pub fn start(gg: GameGirl) {
    let gg = Arc::new(Mutex::new(gg));
    let _stream = setup_cpal(gg.clone());
    init_eframe(gg);
}

fn setup_cpal(gg: Arc<Mutex<GameGirl>>) -> Stream {
    let device = cpal::default_host().default_output_device().unwrap();
    let stream = device
        .build_output_stream(
            &StreamConfig {
                channels: 2,
                sample_rate: SampleRate(SAMPLE_RATE),
                buffer_size: BufferSize::Fixed(1024),
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

fn init_eframe(gg: Arc<Mutex<GameGirl>>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(160.0, 144.0)),
        ..Default::default()
    };
    eframe::run_native(
        "gameGirl",
        options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            gg.lock().unwrap().mmu.ppu.repaint = Box::new(move || ctx.request_repaint());

            let manager = cc.egui_ctx.tex_manager();
            let texture = manager.write().alloc(
                "screen".into(),
                ColorImage::new([160, 144], Colour::BLACK).into(),
            );
            Box::new(App { gg, texture })
        }),
    )
}

struct App {
    gg: Arc<Mutex<GameGirl>>,
    texture: TextureId,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_gg(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.image(self.texture, [160.0, 144.0]);
        });
    }
}

impl App {
    fn update_gg(&mut self, ctx: &egui::Context) {
        let frame = {
            let mut gg = self.gg.lock().unwrap();
            for event in &ctx.input().events {
                if let Event::Key { key, pressed, .. } = event {
                    if let Some(button) = Button::from_key(*key) {
                        Joypad::set(&mut gg, button, *pressed);
                    }
                }
            }
            gg.mmu.ppu.last_frame.take()
        };
        if let Some(data) = frame {
            let img = ImageDelta::full(ImageData::Color(ColorImage {
                size: [160, 144],
                pixels: data,
            }));
            let manager = ctx.tex_manager();
            manager.write().set(self.texture, img);
        }
    }
}
