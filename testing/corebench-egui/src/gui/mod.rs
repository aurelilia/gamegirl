// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

pub mod file_dialog;
mod tests;

use std::fs;

use eframe::egui::{
    self, load::SizedTexture, vec2, widgets, CollapsingHeader, Color32, Context, Image, RichText,
    Ui, ViewportCommand,
};

use self::file_dialog::File;
use crate::{
    app::{App, Message},
    testsuite::TestStatus,
    DCore,
};

/// Function signature for an app window
type AppFn = fn(&mut App, &Context, &mut Ui);
/// Count of GUI windows that take the App as a parameter.
pub const APP_WINDOW_COUNT: usize = 2;
/// GUI windows that take the App as a parameter.
const APP_WINDOWS: [(&str, AppFn); APP_WINDOW_COUNT] =
    [("Test Suites", tests::suites), ("Benchmark", tests::bench)];

pub fn draw(app: &mut App, ctx: &Context, size: [usize; 2]) {
    navbar(app, ctx);
    game_screens(app, ctx, size);

    let mut states = app.app_window_states;
    for ((name, runner), state) in APP_WINDOWS.iter().zip(states.iter_mut()) {
        make_window(app, ctx, name, state, *runner);
    }
    app.app_window_states = states;
}

fn navbar(app: &mut App, ctx: &Context) {
    egui::TopBottomPanel::top("navbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.visuals_mut().button_frame = false;
            navbar_content(app, ctx, ui);
        });
    });
}

fn navbar_content(app: &mut App, ctx: &Context, ui: &mut Ui) {
    widgets::global_dark_light_mode_switch(ui);
    ui.separator();

    ui.menu_button("File", |ui| {
        if ui.button("Open ROM").clicked() {
            file_dialog::open_rom(app.message_channel.0.clone());
            ui.close_menu();
        }
        if ui.button("Open Replay").clicked() {
            file_dialog::open_replay(app.message_channel.0.clone());
            ui.close_menu();
        }
        if ui.button("Open Core").clicked() {
            file_dialog::open_core(app.message_channel.0.clone());
            ui.close_menu();
        }
        ui.separator();

        if ui.button("Exit").clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close)
        }
    });

    if ui.button("Test Suites").clicked() {
        app.app_window_states[0] ^= true;
    }
}

fn game_screens(app: &mut App, ctx: &Context, size: [usize; 2]) {
    let mut remove = None;
    for (i, c) in app.cores.iter().enumerate() {
        let mut keep = true;
        egui::Window::new(&format!("Core: {}", c.name))
            .open(&mut keep)
            .show(ctx, |ui| {
                ui.add(make_screen_ui(app, size, i));

                if ui.button("Copy Screen Hash").clicked() {
                    app.message_channel
                        .0
                        .send(Message::CopyHashToClipboard(i))
                        .unwrap();
                }

                ui.separator();
                ui.heading("Test Suites");
                for (i, suite) in app.suites.iter().enumerate() {
                    let tests = c.suites[i].lock().unwrap();
                    CollapsingHeader::new(RichText::new(&suite.name).strong())
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for test in tests.0.iter() {
                                    let text = match test.result {
                                        TestStatus::Waiting => "😽",
                                        TestStatus::Running => "😼",
                                        TestStatus::Success => "😻",
                                        _ => "😿",
                                    };
                                    let color = match test.result {
                                        TestStatus::Waiting => Color32::WHITE,
                                        TestStatus::Running => Color32::YELLOW,
                                        TestStatus::Success => Color32::GREEN,
                                        _ => Color32::RED,
                                    };
                                    if ui
                                        .label(RichText::new(text).color(color).size(15.0))
                                        .on_hover_text(&test.test.name)
                                        .clicked()
                                    {
                                        app.message_channel
                                            .0
                                            .send(Message::RomOpen(File {
                                                content: test.test.rom.clone(),
                                                path: None,
                                            }))
                                            .unwrap();
                                    }
                                }
                            });
                        });
                    ui.label(
                        RichText::new(format!("Passed: ({}/{})", tests.1, tests.0.len())).italics(),
                    );
                    ui.separator();
                }
            });
        if !keep {
            remove = Some(i);
        }
    }

    if let Some(core) = remove {
        let DCore { _library, name, .. } = app.cores.remove(core);
        drop(_library);
        fs::remove_file(format!("dyn-cores/{name}")).unwrap();
    }
}

fn make_screen_ui(app: &App, size: [usize; 2], idx: usize) -> Image {
    egui::Image::new(Into::<SizedTexture>::into((
        app.textures[idx],
        vec2(size[0] as f32 * 2.0, size[1] as f32 * 2.0),
    )))
    .shrink_to_fit()
}

fn make_window(app: &mut App, ctx: &Context, title: &str, open: &mut bool, content: AppFn) {
    egui::Window::new(title)
        .open(open)
        .show(ctx, |ui| content(app, ctx, ui));
}
