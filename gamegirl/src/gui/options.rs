use core::common::{Button, CgbMode, SystemConfig};

use eframe::{
    egui,
    egui::{vec2, CollapsingHeader, ComboBox, Context, Slider, TextureFilter, Ui},
};
use serde::{Deserialize, Serialize};

use crate::gui::{
    input::{Input, InputAction, HOTKEYS},
    App,
};

/// User-configurable options.
#[derive(Serialize, Deserialize)]
pub struct Options {
    /// Options passed to the system when loading a ROM.
    pub gg: SystemConfig,
    /// Input configuration.
    pub input: Input,

    /// Fast forward speed for the hold button.
    pub fast_forward_hold_speed: usize,
    /// Fast forward speed for the toggle button.
    pub fast_forward_toggle_speed: usize,
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
            fast_forward_hold_speed: 2,
            fast_forward_toggle_speed: 2,
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
        ui.separator();

        ui.horizontal(|ui| {
            ui.add(Slider::new(&mut opt.fast_forward_hold_speed, 2..=10));
            ui.label("Fast forward speed (Hold)");
        });
        ui.horizontal(|ui| {
            ui.add(Slider::new(&mut opt.fast_forward_toggle_speed, 2..=10));
            ui.label("Fast forward speed (Toggle)");
        });
        ui.separator();

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
                    let size = app.gg.lock().unwrap().screen_size();
                    app.texture = App::make_screen_texture(ctx, size, opt.tex_filter);
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
                app.gg.lock().unwrap().config_mut().volume = opt.gg.volume;
            }
            ui.label("Volume");
        });
    });

    input_section(
        ui,
        "Input",
        opt,
        Button::BUTTONS
            .iter()
            .map(|btn| (format!("{:?}", btn), InputAction::Button(*btn))),
    );
    input_section(
        ui,
        "Hotkeys",
        opt,
        HOTKEYS
            .iter()
            .enumerate()
            .map(|(i, (n, _))| (n.to_string(), InputAction::Hotkey(i as u8))),
    );

    ui.separator();
    ui.label("Some options require a restart to apply.");
}

fn input_section(
    ui: &mut Ui,
    name: &'static str,
    opt: &mut Options,
    iter: impl Iterator<Item = (String, InputAction)>,
) {
    CollapsingHeader::new(name).show(ui, |ui| {
        for (name, action) in iter {
            let active = Some(action) == opt.input.pending;
            let text = if active {
                "...".to_string()
            } else {
                opt.input.key_for_fmt(action)
            };

            ui.horizontal(|ui| {
                if ui
                    .add_sized(vec2(90.0, 20.0), egui::Button::new(text))
                    .clicked()
                {
                    opt.input.pending = Some(action);
                }
                ui.label(name);
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
