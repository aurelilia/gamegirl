mod gui;

use std::fs;

use gamegirl::{GameCart, SystemConfig};
use gtk::{
    Application, ApplicationWindow,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{self},
    prelude::{GtkWindowExt, WidgetExt, WidgetExtManual},
};
use gui::canvas::GamegirlPaintable;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("eu.catin.gamegirl")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let core = gamegirl::load_cart(
        GameCart {
            rom: fs::read("bench.gb").unwrap(),
            save: None,
        },
        &SystemConfig::default(),
    )
    .unwrap();

    let draw = GamegirlPaintable::new(core);
    let pict = gtk::Picture::builder()
        .halign(gtk::Align::BaselineFill)
        .valign(gtk::Align::BaselineFill)
        .content_fit(gtk::ContentFit::Contain)
        .hexpand(true)
        .vexpand(true)
        .paintable(&draw)
        .build();
    pict.add_tick_callback(|pict, _| {
        pict.queue_draw();
        glib::ControlFlow::Continue
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .title("gamegirl")
        .child(&pict)
        .build();
    window.present();
}
