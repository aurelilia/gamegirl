use eframe::epaint::{ColorImage, ImageDelta, TextureId};

use crate::egui::{Color32, ImageData};
use crate::{egui, GameGirl};

pub type Colour = Color32;

pub fn start(gg: GameGirl) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(160.0, 144.0)),
        ..Default::default()
    };
    eframe::run_native(
        "gameGirl",
        options,
        Box::new(|cc| {
            let manager = cc.egui_ctx.tex_manager();
            let texture = manager.write().alloc(
                "screen".into(),
                ColorImage::new([160, 144], Colour::BLACK).into(),
            );
            Box::new(App { gg, texture })
        }),
    );
}

struct App {
    gg: GameGirl,
    texture: TextureId,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.gg.advance_delta(0.1);

        let data = self.gg.mmu.ppu.pixels.to_vec();
        let img = ImageDelta::full(ImageData::Color(ColorImage {
            size: [160, 144],
            pixels: data,
        }));
        let manager = ctx.tex_manager();
        manager.write().set(self.texture, img);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.image(self.texture, [160.0, 144.0]);
        });
    }
}
