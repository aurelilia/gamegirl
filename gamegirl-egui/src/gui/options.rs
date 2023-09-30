// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::misc::{Button, CgbMode, SystemConfig};
use eframe::{
    egui,
    egui::{vec2, CollapsingHeader, ComboBox, Context, Slider, TextureFilter, Ui},
};

use crate::gui::{
    input::{Input, InputAction, HOTKEYS},
    App,
};

/// User-configurable options.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
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
        ui.checkbox(&mut opt.gg.cached_interpreter, "GGA: Enable Cached Interpreter")
            .on_hover_text("Enables caching in the interpreter. Speeds up emulation at the cost of RAM usage. Also breaks breakpoints.");
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

        #[cfg(feature = "savestates")]
        {
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
                ui.selectable_value(&mut opt.tex_filter, TextureFilter::Nearest, "Nearest");
                ui.selectable_value(&mut opt.tex_filter, TextureFilter::Linear, "Linear");
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
        ui.label("GameGirl v0.1.1 made by");
        ui.hyperlink_to("leela aurelia", "https://elia.garden");
    });
    ui.horizontal(|ui| {
        ui.label("Based on my previous emulator");
        ui.hyperlink_to("gamelin", "https://git.elia.garden/leela/gamelin");
    });
    ui.horizontal(|ui| {
        ui.label("Made possible thanks to many amazing people. <3");
    });
}
