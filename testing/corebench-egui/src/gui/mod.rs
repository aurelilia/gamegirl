// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod file_dialog;

use eframe::egui::{self, load::SizedTexture, vec2, widgets, Context, Image, Ui, ViewportCommand};

use crate::app::App;

/// Function signature for an app window
type AppFn = fn(&mut App, &Context, &mut Ui);

pub fn draw(app: &mut App, ctx: &Context, size: [usize; 2]) {
    navbar(app, ctx);
    game_screens(app, ctx, size);
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
}

fn game_screens(app: &App, ctx: &Context, size: [usize; 2]) {
    for (i, c) in app.cores.iter().enumerate() {
        egui::Window::new(&format!("Core: {}", c.name))
            .show(ctx, |ui| ui.add(make_screen_ui(app, size, i)));
    }
}

fn make_screen_ui(app: &App, size: [usize; 2], idx: usize) -> Image {
    egui::Image::new(Into::<SizedTexture>::into((
        app.textures[idx],
        vec2(size[0] as f32, size[1] as f32),
    )))
    .shrink_to_fit()
}

fn make_window(app: &mut App, ctx: &Context, title: &str, open: &mut bool, content: AppFn) {
    egui::Window::new(title)
        .open(open)
        .show(ctx, |ui| content(app, ctx, ui));
}
