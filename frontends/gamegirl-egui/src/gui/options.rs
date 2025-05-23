// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use eframe::{
    egui,
    egui::{vec2, CollapsingHeader, ComboBox, Context, Slider, Ui},
};
use egui::{Color32, RichText, Separator};
use gamegirl::{
    common::common::{audio::AudioSampler, input::Button, options::CgbMode},
    frontend::input::InputAction,
};

use crate::{
    app::{App, GuiStyle, Options},
    filter::{Blend, Filter},
    input::{file_dialog, HOTKEYS},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Emulation,
    Features,
    Gui,
    Audio,
    Input,
    About,
}

impl Panel {
    pub const ALL: [(&'static str, Panel); 6] = [
        ("🖭 Emulation", Panel::Emulation),
        ("❇ Features", Panel::Features),
        ("🖵 GUI", Panel::Gui),
        ("🔊 Audio", Panel::Audio),
        ("🎮 Input", Panel::Input),
        ("😼 About", Panel::About),
    ];
}

/// Show the options menu.
pub(super) fn options(app: &mut App, ctx: &Context, ui: &mut Ui) {
    let opt = &mut app.state.options;

    ui.horizontal(|ui| {
        for (name, panel) in Panel::ALL {
            ui.selectable_value(&mut app.open_option, panel, name);
        }
    });
    ui.separator();

    match app.open_option {
        Panel::Emulation => {
            ui.heading("Console BIOSes");
            ui.label("Select the BIOS files to use for the emulated consoles.");
            ui.label("All 3 GameBoy consoles will use a replacement BIOS if not provided.");
            ui.label(
                RichText::new("Some GBA games need an official BIOS to work.")
                    .color(Color32::YELLOW),
            );
            egui::Grid::new("bios").spacing((8.0, 4.0)).show(ui, |ui| {
                for bios in opt.sys.bioses.iter_mut() {
                    ui.strong(&bios.console_name);
                    if bios.bios.is_some() {
                        ui.label(RichText::new("    ✅ Using provided BIOS").color(Color32::GREEN));
                        if ui.button("Clear").clicked() {
                            bios.bios = None;
                        }
                    } else {
                        ui.label("    ❌ No BIOS provided");
                        if ui.button("Load").clicked() {
                            file_dialog::open_bios(
                                app.message_channel.0.clone(),
                                bios.console_id.clone(),
                            );
                        }
                    }
                    ui.end_row();
                }
            });
            ui.add(Separator::default().spacing(10.));

            ui.heading("GameBoy");
            ComboBox::from_label("GB Color mode")
                .selected_text(format!("{:?}", opt.sys.mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut opt.sys.mode, CgbMode::Always, "Always");
                    ui.selectable_value(&mut opt.sys.mode, CgbMode::Prefer, "Prefer");
                    ui.selectable_value(&mut opt.sys.mode, CgbMode::Never, "Never");
                });
            ui.checkbox(
                &mut opt.sys.cgb_colour_correction,
                "Enable GBC colour correction",
            )
            .on_hover_text("Adjust colours to be more accurate to a real GBC screen.");
            ui.add(Separator::default().spacing(10.));

            ui.heading("Gameboy Advance");
            ui.checkbox(&mut opt.sys.cached_interpreter, "Enable Cached Interpreter")
            .on_hover_text("Enables caching in the interpreter. Speeds up emulation at the cost of RAM usage. Also breaks breakpoints.");

            #[cfg(not(target_arch = "wasm32"))]
            ui.checkbox(&mut opt.sys.threaded_ppu, "Enable Threaded Graphics")
                .on_hover_text("Enables running the GGA PPU in a separate thread. Speeds up emulation a lot, but uses slightly more CPU and RAM and might cause some subtle graphical glitches.");
        }

        Panel::Features => {
            ui.heading("General");
            ui.checkbox(&mut opt.sys.run_on_open, "Start running on ROM load")
                .on_hover_text(
                    "Immediately start running the emulation as soon as a ROM is loaded.",
                );
            ui.checkbox(
                &mut opt.sys.skip_bootrom,
                "Skip System ROM / BIOS",
            )
            .on_hover_text("Skip any kind of intro the system would usually play (e.g. 'GameBoy' logo splash) and run the game immediately.");
            ui.add(Separator::default().spacing(10.));

            ui.heading("Fast-forward");
            ui.horizontal(|ui| {
                ui.add(Slider::new(
                    &mut opt.rewinder.ff_hold_speed,
                    2..=10,
                ));
                ui.label("Fast forward speed (Hold)");
            });
            ui.horizontal(|ui| {
                ui.add(Slider::new(
                    &mut opt.rewinder.ff_toggle_speed,
                    2..=10,
                ));
                ui.label("Fast forward speed (Toggle)");
            });
            ui.add(Separator::default().spacing(10.));

            ui.heading("Rewind");
            ui.checkbox(&mut opt.rewinder.enable_rewind, "Enable Rewinding");
            if opt.rewinder.enable_rewind {
                ui.checkbox(&mut opt.sys.compress_savestates, "Compress rewind data")
                .on_hover_text("Heavily reduces rewinding memory usage, but requires a lot of performance.\nLoad a ROM to apply changes to this.");
                ui.horizontal(|ui| {
                    ui.add(Slider::new(&mut opt.rewinder.rewind_buffer_size, 1..=60))
                        .on_hover_text(format!(
                            "Uses about ~{}MB of RAM",
                            opt.rewinder.rewind_buffer_size
                                + opt.rewinder.rewind_buffer_size
                                    * (!opt.sys.compress_savestates as usize * 4),
                        ));
                    ui.label("Rewind time in seconds");
                });
            }
        }

        Panel::Gui => {
            ui.heading("Graphics");
            ComboBox::from_label("Texture filter")
                .selected_text(format!("{:?}", opt.tex_filter))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut opt.tex_filter, Filter::Nearest, "Nearest");
                    ui.selectable_value(&mut opt.tex_filter, Filter::Linear, "Linear");
                    ui.selectable_value(&mut opt.tex_filter, Filter::Hq2x, "hq2x");
                    ui.selectable_value(&mut opt.tex_filter, Filter::Hq3x, "hq3x");
                    ui.selectable_value(&mut opt.tex_filter, Filter::Hq4x, "hq4x");
                });
            ComboBox::from_label("Blending mode")
                .selected_text(format!("{:?}", opt.screen_blend))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut opt.screen_blend, Blend::None, "None")
                        .on_hover_text("No blending, just draw the current frame.");
                    ui.selectable_value(&mut opt.screen_blend, Blend::Soften, "Soften")
                        .on_hover_text("Blend the last 2 frames, simulating light ghosting. Can fix flickering in some games.");
                    ui.selectable_value(&mut opt.screen_blend, Blend::Accumulate, "Accumulate")
                        .on_hover_text("Accumulatively blend past frames. Can fix flickering in some games.");
                });
            ui.horizontal(|ui| {
                let mut core = app.core.lock().unwrap();
                ui.add(Slider::new(
                    &mut core.c_mut().video_buffer.frameskip,
                    0..=10,
                ));
                ui.label("Frameskip");
            });

            ui.checkbox(&mut opt.pixel_perfect, "Pixel perfect scaling")
            .on_hover_text("Will only scale the screen to integer multiples, preventing some scaling artifacts at the cost of screen size.\nMainly applicable with 'Nearest' filtering.");
            ui.checkbox(&mut opt.preserve_aspect_ratio, "Preserve aspect ratio")
                .on_hover_text("Will keep the aspect ratio of the original screen.");
            ui.add(Separator::default().spacing(10.));

            ui.heading("GUI");
            ComboBox::from_label("GUI Style")
                .selected_text(format!("{:?}", opt.gui_style))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut opt.gui_style,
                        GuiStyle::OnTop,
                        "Windows on top of game",
                    );
                    ui.selectable_value(
                        &mut opt.gui_style,
                        GuiStyle::AllWindows,
                        "Everything in windows",
                    );
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.selectable_value(
                        &mut opt.gui_style,
                        GuiStyle::MultiWindow,
                        "Multiple native windows",
                    );
                });

            CollapsingHeader::new("egui Configuration").show(ui, |ui| ctx.settings_ui(ui));
        }

        Panel::Audio => {
            ui.horizontal(|ui| {
                if ui
                    .add(Slider::new(&mut opt.sys.volume, 0.0..=1.0))
                    .changed()
                {
                    app.core.lock().unwrap().c_mut().config.volume = opt.sys.volume;
                }
                ui.label("Volume");
            });
            ui.horizontal(|ui| {
                if ui
                    .add(Slider::new(&mut opt.sys.volume_ff, 0.0..=1.0))
                    .changed()
                {
                    app.core.lock().unwrap().c_mut().config.volume_ff = opt.sys.volume_ff;
                }
                ui.label("Volume during Fast-Forward");
            });

            ComboBox::from_label("Output Sample Rate")
                .selected_text(format!("{:.1}kHz", opt.sys.sample_rate / 1000))
                .show_ui(ui, |ui| {
                    for sr in [22050, 44100, 48000, 96000, 192000].iter() {
                        ui.selectable_value(
                            &mut opt.sys.sample_rate,
                            *sr,
                            format!("{:.1}kHz", *sr as f32 / 1000.),
                        );
                    }
                })
                .response
                .on_hover_text("Requires restart to apply changes.");

            ComboBox::from_label("Resampling Algorithm")
                .selected_text(format!("{}", opt.sys.resampler))
                .show_ui(ui, |ui| {
                    for sampler in AudioSampler::SAMPLERS.iter() {
                        if ui
                            .selectable_value(
                                &mut opt.sys.resampler,
                                *sampler,
                                format!("{}", sampler),
                            )
                            .changed()
                        {
                            app.core
                                .lock()
                                .unwrap()
                                .c_mut()
                                .audio_buffer
                                .set_sampling(*sampler);
                        };
                    }
                });
        }

        Panel::Input => {
            ui.horizontal(|ui| {
                input_section(
                    ui,
                    opt,
                    Button::BUTTONS
                        .iter()
                        .map(|btn| (format!("{:?}", btn), InputAction::Button(*btn))),
                );
                ui.separator();
                input_section(
                    ui,
                    opt,
                    HOTKEYS
                        .iter()
                        .enumerate()
                        .map(|(i, (n, _))| (n.to_string(), InputAction::Hotkey(i as u8))),
                );
            });
        }

        Panel::About => {
            ui.horizontal(|ui| {
                ui.label("GameGirl v0.2.0 made by");
                ui.hyperlink_to("leela aurelia", "https://elia.garden");
            });
            ui.horizontal(|ui| {
                ui.label("Based on my previous emulator");
                ui.hyperlink_to("gamelin", "https://git.elia.garden/leela/gamelin");
            });
            ui.horizontal(|ui| {
                ui.label("Made possible thanks to many amazing people. <3");
            });

            ui.add(Separator::default().spacing(10.));

            ui.heading("Credit for downloadable games");
            for game in super::DEMO_APP_URLS {
                ui.horizontal(|ui| {
                    ui.label(game.0);
                    ui.hyperlink_to("Source", game.2);
                });
            }
        }
    }
}

fn input_section(
    ui: &mut Ui,
    opt: &mut Options,
    iter: impl Iterator<Item = (String, InputAction)>,
) {
    ui.vertical(|ui| {
        for (name, action) in iter {
            let active = Some(action) == opt.input.pending;
            let text = opt.input.key_for_fmt(action);
            let text = match active {
                true if !text.is_empty() => format!("{text}, ..."),
                true => "...".to_string(),
                false => text,
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
