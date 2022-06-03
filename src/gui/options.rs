use crate::gui::State;
use crate::system::{CgbMode, GGOptions};
use eframe::egui::{CollapsingHeader, ComboBox, Context, Slider, Ui};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Options {
    pub gg: GGOptions,
    pub enable_rewind: bool,
    pub rewind_buffer_size: usize,
}

pub fn options(ctx: &Context, state: &mut State, ui: &mut Ui) {
    CollapsingHeader::new("Emulation").show(ui, |ui| {
        ComboBox::from_label("GB Colour mode")
            .selected_text(format!("{:?}", state.options.gg.mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.options.gg.mode, CgbMode::Always, "Always");
                ui.selectable_value(&mut state.options.gg.mode, CgbMode::Prefer, "Prefer");
                ui.selectable_value(&mut state.options.gg.mode, CgbMode::Never, "Never");
            });

        ui.checkbox(&mut state.options.enable_rewind, "Enable Rewinding");
        if state.options.enable_rewind {
            ui.horizontal(|ui| {
                ui.label("Rewind buffer size: ");
                ui.add(Slider::new(&mut state.options.rewind_buffer_size, 1..=60));
                ui.label(format!(
                    "({}s, ~{}MB)",
                    state.options.rewind_buffer_size, state.options.rewind_buffer_size
                ));
            });
        }
    });

    CollapsingHeader::new("egui Configuration").show(ui, |ui| ctx.settings_ui(ui));
}

pub fn about(_ctx: &Context, _state: &mut State, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label("GameGirl v0.1 made by ");
        ui.hyperlink_to("ellie leela", "https://angm.xyz");
    });
    ui.horizontal(|ui| {
        ui.label("Based on my previous emulator ");
        ui.hyperlink_to("gamelin", "https://git.angm.xyz/ellie/gamelin");
    });
    ui.horizontal(|ui| {
        ui.label("Made possible thanks to many amazing people. <3");
    });
}
