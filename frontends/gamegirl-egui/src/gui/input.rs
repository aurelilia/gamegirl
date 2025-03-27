use egui::{Button, Context, Frame, Margin, RichText, Sense, Ui};
use gamegirl::common::{common::input, Core};

use crate::App;

pub fn render(app: &mut App, ctx: &Context) {
    let mut core = app.core.lock().unwrap();
    button_win(ctx, &mut core, input::Button::A, "A");
    button_win(ctx, &mut core, input::Button::B, "B");
    button_win(ctx, &mut core, input::Button::L, "L");
    button_win(ctx, &mut core, input::Button::R, "R");
    button_win(ctx, &mut core, input::Button::Start, "START");
    button_win(ctx, &mut core, input::Button::Select, "SELECT");

    egui::Window::new("dpad")
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("dpadgrid").show(ui, |ui| {
                ui.label("");
                button_ui(ui, &mut core, input::Button::Up, "^");
                ui.label("");
                ui.end_row();

                button_ui(ui, &mut core, input::Button::Left, "<");
                ui.label("");
                button_ui(ui, &mut core, input::Button::Right, ">");
                ui.end_row();

                ui.label("");
                button_ui(ui, &mut core, input::Button::Down, "v");
                ui.label("");
                ui.end_row();
            });
        });
}

fn button_win(ctx: &Context, core: &mut Box<dyn Core>, button: input::Button, text: &str) {
    egui::Window::new(text)
        .title_bar(false)
        .resizable(false)
        .frame(Frame::window(&ctx.style()).inner_margin(Margin::same(10)))
        .show(ctx, |ui| button_ui(ui, core, button, text));
}

fn button_ui(ui: &mut Ui, core: &mut Box<dyn Core>, button: input::Button, text: &str) {
    ui.spacing_mut().button_padding = [10.0, 10.0].into();
    let btn = ui.add(
        Button::new(RichText::new(text).size(40.0))
            .corner_radius(50.0)
            .sense(Sense::drag())
            .min_size([60.0; 2].into()),
    );

    if btn.drag_stopped() {
        core.c_mut().input.set(0, button, false);
    }
    if btn.drag_started() {
        core.c_mut().input.set(0, button, true);
    }
}
