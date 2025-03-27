// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

pub mod cheat;
mod input;
pub mod options;

use std::{fs, time::Duration};

use eframe::{
    egui::{self, load::SizedTexture, vec2, widgets, Context, Image, Layout, Ui, ViewportCommand},
    emath::Align,
    epaint::Vec2,
    Frame,
};
use file_dialog::File;
use gamegirl::common::common::input::{InputReplay, ReplayState};

use crate::{
    app::{App, GuiStyle, Message},
    debug,
    input::file_dialog,
};

const DEMO_APP_URLS: &[(&str, &str, &str)] = &[
    (
        "Apotris",
        "https://gg.elia.garden/game/Apotris.gba",
        "https://akouzoukos.com/apotris",
    ),
    (
        "Celeste Classic",
        "https://gg.elia.garden/game/CelesteClassic.gba",
        "https://github.com/JeffRuLz/Celeste-Classic-GBA",
    ),
    (
        "Feline",
        "https://gg.elia.garden/game/feline.gba",
        "https://github.com/foopod/gbaGamejam2021",
    ),
];

/// Function signature for an app window
type AppFn = fn(&mut App, &Context, &mut Ui);
/// Count of GUI windows that take the App as a parameter.
pub const APP_WINDOW_COUNT: usize = 3;
/// GUI windows that take the App as a parameter.
const APP_WINDOWS: [(&str, AppFn); APP_WINDOW_COUNT] = [
    ("Options", options::options),
    ("Replays", replays),
    ("Cheat Engine", cheat::ui),
];

pub fn draw(app: &mut App, ctx: &Context, frame: &Frame, size: [usize; 2]) {
    ctx.style_mut(|s| s.spacing.item_spacing[1] = 5.);
    navbar(app, ctx, frame);
    game_screen(app, ctx, size);

    let mut states = app.app_window_states;
    for ((name, runner), state) in APP_WINDOWS.iter().zip(states.iter_mut()) {
        make_window(app, ctx, name, state, *runner);
    }
    app.app_window_states = states;

    if app.on_screen_input {
        input::render(app, ctx);
    }
    debug::render(app, ctx);
    app.toasts.show(ctx);
}

fn navbar(app: &mut App, ctx: &Context, frame: &Frame) {
    egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.visuals_mut().button_frame = false;
            let now = { ctx.input(|i| i.time) };
            navbar_content(app, now, frame, ctx, ui);
        });
    });
}

fn navbar_content(app: &mut App, now: f64, frame: &Frame, ctx: &Context, ui: &mut Ui) {
    widgets::global_theme_preference_switch(ui);
    ui.separator();
    ui.spacing_mut().item_spacing[0] = 13.0;

    ui.menu_button("🗁 File", |ui| {
        if ui.button("📂 Open ROM...").clicked() {
            file_dialog::open_rom(app.message_channel.0.clone());
            ui.close_menu();
        }
        if !app.state.last_opened.is_empty() {
            ui.menu_button("🕐 Last Opened", |ui| {
                for path in &app.state.last_opened {
                    if ui
                        .button(path.file_name().unwrap().to_str().unwrap())
                        .clicked()
                    {
                        app.message_channel
                            .0
                            .send(Message::RomOpen(File {
                                content: fs::read(path).unwrap(),
                                path: Some(path.clone()),
                            }))
                            .ok();
                        ui.close_menu();
                    }
                }
            });
        }

        ui.menu_button("🌐 Download Game", |ui| {
            for (name, url, _) in DEMO_APP_URLS {
                if ui.button(*name).clicked() {
                    app.toasts
                        .info(format!("Downloading {url}..."))
                        .duration(Some(Duration::from_secs(5)));

                    let request = ehttp::Request::get(url);
                    let tx = app.message_channel.0.clone();
                    ehttp::fetch(
                        request,
                        move |result: ehttp::Result<ehttp::Response>| match result {
                            Ok(r) => tx
                                .send(Message::RomOpen(File {
                                    content: r.bytes,
                                    path: None,
                                }))
                                .unwrap(),
                            Err(err) => {
                                tx.send(Message::Error(format!("Download failed: {err:?}...")))
                                    .unwrap();
                            }
                        },
                    );
                }
            }
        });
        ui.separator();

        if ui.button("💾 Save").clicked() {
            app.save_game();
            app.toasts
                .info("Game saved")
                .duration(Some(Duration::from_secs(5)));
            ui.close_menu();
        }
        if ui.button("💾 Save As...").clicked() {
            let save = { app.core.lock().unwrap().make_save() };
            if let Some(file) = save {
                file_dialog::save_gamesave(file.title, file.ram);
                app.toasts
                    .info(format!("Game saved"))
                    .duration(Some(Duration::from_secs(5)));
            }
            ui.close_menu();
        }

        let text = if app.core.lock().unwrap().c().debugger.running {
            "⏸ Pause"
        } else {
            "▶ Resume"
        };
        if ui.button(text).clicked() {
            app.pause();
            ui.close_menu();
        }

        if ui.button("↺ Reset").clicked() {
            app.reset();
            ui.close_menu();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.separator();
            if ui.button("🚪 Exit").clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Close)
            }
        }
    });

    ui.menu_button("✨ Features", |ui| {
        if ui.button("⏪ Replays").clicked() {
            app.app_window_states[1] ^= true;
            ui.close_menu();
        }

        if ui.button("🐲 Cheat Engine").clicked() {
            app.app_window_states[2] ^= true;
            ui.close_menu();
        }

        ui.menu_button("🖴 Savestates", |ui| {
            for (i, state) in app.rewinder.save_states.iter_mut().enumerate() {
                if ui.button(format!("↘ Save State {}", i + 1)).clicked() {
                    *state = Some(app.core.lock().unwrap().save_state());
                    app.toasts
                        .info(format!("Saved state {}", i + 1))
                        .duration(Some(Duration::from_secs(3)));
                    ui.close_menu();
                }
            }
            ui.separator();

            for (i, state) in app
                .rewinder
                .save_states
                .iter()
                .filter_map(|s| s.as_ref())
                .enumerate()
            {
                if ui.button(format!("↗ Load State {}", i + 1)).clicked() {
                    let mut core = app.core.lock().unwrap();
                    app.rewinder.before_last_ss_load = Some(core.save_state());
                    core.load_state(state);
                    app.toasts
                        .info(format!("Loaded state {}", i + 1))
                        .duration(Some(Duration::from_secs(3)));
                    ui.close_menu();
                }
            }
        });

        ui.menu_button("🐛 Debugger", |ui| debug::menu(app, ui));
    });

    ui.menu_button("☸ Options", |ui| {
        for (name, panel) in options::Panel::ALL {
            if ui.button(name).clicked() {
                app.app_window_states[0] ^= true;
                app.open_option = panel;
                ui.close_menu();
            }
        }
    });

    if ui.button("🖱 On-screen Input").clicked() {
        app.on_screen_input ^= true;
    }

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let time = frame.info().cpu_usage.unwrap_or(0.0);
        app.frame_times.add(now, time);
        // Backwards because we're in RTL layout
        ui.monospace(format!(
            "{:.3}ms",
            app.frame_times.average().unwrap_or(0.0) * 1000.0
        ));
        ui.label("Frame time: ");
    });
}

fn replays(app: &mut App, _ctx: &Context, ui: &mut Ui) {
    let mut core = app.core.lock().unwrap();
    match (&core.c_mut().input.replay, app.current_rom_path.clone()) {
        (ReplayState::None, None) => {
            ui.label("Status: Not currently recording replay");
            ui.label("Hint: Load a ROM first.");
        }

        (ReplayState::None, Some(file)) => {
            ui.label("Status: Not currently recording replay");
            if ui.button("Restart system and start recording").clicked() {
                core.c_mut().input.replay = ReplayState::Recording(InputReplay::empty(
                    file.as_os_str().to_string_lossy().into(),
                ));
                core.reset();
            }
            if ui.button("Load recording and restart").clicked() {
                file_dialog::open_replay(app.message_channel.0.clone());
            }
        }

        (ReplayState::Recording(ir), _) => {
            ui.label("Status: Recording replay");
            ui.label(&format!("Recorded {} states!", ir.states.len()));
            if ui.button("End & Save Replay").clicked() {
                file_dialog::save_replay(ir.serialize());
                core.c_mut().input.replay = ReplayState::None;
            }
        }

        (ReplayState::Playback(_), _) => {
            ui.label("Status: Playing back replay");
            if ui.button("End Playback").clicked() {
                core.c_mut().input.replay = ReplayState::None;
            }
        }
    }
}

fn game_screen(app: &App, ctx: &Context, size: [usize; 2]) {
    match app.state.options.gui_style {
        GuiStyle::AllWindows => {
            egui::Window::new("Screen").show(ctx, |ui| {
                ui.add(make_screen_ui(app, size, ui.available_size()))
            });
        }
        GuiStyle::OnTop | GuiStyle::MultiWindow => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.add(make_screen_ui(app, size, ui.available_size()))
                });
            });
        }
    }
}

fn make_screen_ui(app: &App, size: [usize; 2], avail_size: Vec2) -> Image {
    if app.state.options.pixel_perfect {
        // Find the biggest multiple of the screen size that still fits
        let orig_size = Vec2::new(size[0] as f32, size[1] as f32);
        let mut size = orig_size + orig_size;
        while size.x < avail_size.x && size.y < avail_size.y {
            size += orig_size;
        }
        size -= orig_size;

        egui::Image::new(Into::<SizedTexture>::into((app.textures[0], size)))
    } else {
        egui::Image::new(Into::<SizedTexture>::into((
            app.textures[0],
            vec2(size[0] as f32, size[1] as f32),
        )))
        .maintain_aspect_ratio(app.state.options.preserve_aspect_ratio)
        .shrink_to_fit()
    }
}

fn make_window(app: &mut App, ctx: &Context, title: &str, open: &mut bool, content: AppFn) {
    match app.state.options.gui_style {
        GuiStyle::OnTop | GuiStyle::AllWindows => {
            egui::Window::new(title)
                .open(open)
                .show(ctx, |ui| content(app, ctx, ui));
        }
        GuiStyle::MultiWindow => {
            if *open {
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of(title),
                    egui::ViewportBuilder::default().with_title(title),
                    |ctx, _| {
                        egui::CentralPanel::default().show(ctx, |ui| content(app, ctx, ui));
                        *open &= !ctx.input(|i| i.viewport().close_requested());
                    },
                )
            }
        }
    }
}
