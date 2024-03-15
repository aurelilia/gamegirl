// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod file_dialog;
mod tests;

use eframe::{
    egui::{
        self, load::SizedTexture, vec2, widgets, Color32, Context, Image, Label, Ui,
        ViewportCommand,
    },
    glow::COLOR,
};

use crate::{app::App, testsuite::TestStatus};

/// Function signature for an app window
type AppFn = fn(&mut App, &Context, &mut Ui);
/// Count of GUI windows that take the App as a parameter.
pub const APP_WINDOW_COUNT: usize = 1;
/// GUI windows that take the App as a parameter.
const APP_WINDOWS: [(&str, AppFn); APP_WINDOW_COUNT] = [("Test Suites", tests::suites)];

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

fn game_screens(app: &App, ctx: &Context, size: [usize; 2]) {
    for (i, c) in app.cores.iter().enumerate() {
        egui::Window::new(&format!("Core: {}", c.name)).show(ctx, |ui| {
            ui.add(make_screen_ui(app, size, i));

            ui.separator();
            ui.heading("Test Suites");
            for (i, suite) in app.suites.iter().enumerate() {
                ui.label(&suite.name);
                ui.horizontal_wrapped(|ui| {
                    let tests = c.suites[i].lock().unwrap();
                    for test in tests.iter() {
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
                        ui.label(egui::RichText::new(text).color(color).size(15.0))
                            .on_hover_text(&test.test.name);
                    }
                });
            }
        });
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
