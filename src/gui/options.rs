use crate::gui::input::{Input, InputAction};
use crate::gui::App;
use crate::system::io::joypad::Button;
use crate::system::{CgbMode, GGOptions};
use eframe::egui::{CollapsingHeader, ComboBox, Context, Slider, TextureFilter, Ui};
use serde::{Deserialize, Serialize};

/// User-configurable options.
#[derive(Serialize, Deserialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub gg: GGOptions,
    /// Input configuration.
    pub input: Input,

    /// Enable rewinding.
    pub enable_rewind: bool,
    /// Rewind buffer size (if enabled), in seconds.
    pub rewind_buffer_size: usize,

    /// Scale of the GG display.
    pub display_scale: usize,
    /// Texture filter applied to the display.
    pub tex_filter: TextureFilter,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            gg: Default::default(),
            input: Input::new(),
            enable_rewind: true,
            rewind_buffer_size: 10,
            display_scale: 2,
            tex_filter: TextureFilter::Nearest,
        }
    }
}

/// Show the options menu.
pub(super) fn options(app: &mut App, ctx: &Context, ui: &mut Ui) {
    let opt = &mut app.state.options;
    CollapsingHeader::new("Emulation").show(ui, |ui| {
        ComboBox::from_label("GB Colour mode")
            .selected_text(format!("{:?}", opt.gg.mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut opt.gg.mode, CgbMode::Always, "Always");
                ui.selectable_value(&mut opt.gg.mode, CgbMode::Prefer, "Prefer");
                ui.selectable_value(&mut opt.gg.mode, CgbMode::Never, "Never");
            });

        ui.checkbox(&mut opt.gg.compress_savestates, "Compress save states/rewinding")
            .on_hover_text("Heavily reduces rewinding memory usage, but requires a lot of performance.\nLoad a ROM to apply changes to this.");
        ui.checkbox(&mut opt.enable_rewind, "Enable Rewinding");
        if opt.enable_rewind {
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut opt.rewind_buffer_size, 1..=60))
                    .on_hover_text(format!(
                        "Uses about ~{}MB of RAM",
                        opt.rewind_buffer_size + opt.rewind_buffer_size * (!opt.gg.compress_savestates as usize * 4),
                    ));
                ui.label("Rewind time in seconds");
            });
        }
    });

    CollapsingHeader::new("Graphics").show(ui, |ui| {
        ui.checkbox(
            &mut opt.gg.cgb_colour_correction,
            "Enable GBC colour correction",
        )
        .on_hover_text("Adjust colours to be more accurate to a real GBC screen.");

        ComboBox::from_label("Texture filter")
            .selected_text(format!("{:?}", opt.tex_filter))
            .show_ui(ui, |ui| {
                let a = ui
                    .selectable_value(&mut opt.tex_filter, TextureFilter::Nearest, "Nearest")
                    .changed();
                let b = ui
                    .selectable_value(&mut opt.tex_filter, TextureFilter::Linear, "Linear")
                    .changed();
                if a || b {
                    app.texture = App::make_screen_texture(ctx, opt.tex_filter);
                }
            });

        ui.horizontal(|ui| {
            ui.add(Slider::new(&mut opt.display_scale, 1..=10));
            ui.label("Screen scale");
        });

        CollapsingHeader::new("egui Configuration").show(ui, |ui| ctx.settings_ui(ui));
    });

    CollapsingHeader::new("Audio").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.add(Slider::new(&mut opt.gg.volume, 0.0..=1.0)).changed() {
                app.gg.lock().unwrap().config.volume = opt.gg.volume;
            }
            ui.label("Volume");
        });
    });

    // TODO buttons are misaligned
    CollapsingHeader::new("Input").show(ui, |ui| {
        for btn in &Button::BUTTONS {
            ui.horizontal(|ui| {
                ui.label(format!("{:?}", btn));
                ui.button(opt.input.key_for_fmt(InputAction::Button(*btn)));
            });
        }
    });
}

/// Show a nice little "About" window. c:
pub(super) fn about(_app: &mut App, _ctx: &Context, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label("GameGirl v0.1 made by");
        ui.hyperlink_to("ellie leela", "https://angm.xyz");
    });
    ui.horizontal(|ui| {
        ui.label("Based on my previous emulator");
        ui.hyperlink_to("gamelin", "https://git.angm.xyz/ellie/gamelin");
    });
    ui.horizontal(|ui| {
        ui.label("Made possible thanks to many amazing people. <3");
    });
}
