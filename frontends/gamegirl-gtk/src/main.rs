mod config;
mod gui;

use std::path::PathBuf;

use adw::{Application, subclass::prelude::ObjectSubclassIsExt};
use config::Options;
use gamegirl::frontend::rewinder::Rewinder;
use gtk::{
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{self},
    prelude::{GtkWindowExt, WidgetExt},
};
use gui::window::GameGirlWindow;

/// App state. This struct is contained in the main app window.
pub struct AppState {
    /// User config state.
    options: Options,
    /// Path of the currently loaded ROM.
    current_rom_path: Option<PathBuf>,
    /// Current rewinder state.
    rewinder: Rewinder<5>,
}

impl Default for AppState {
    fn default() -> Self {
        let options = Options::default();
        let rewinder = Rewinder::new(options.rewinder.rewind_buffer_size);
        Self {
            options,
            current_rom_path: Default::default(),
            rewinder,
        }
    }
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("eu.catin.gamegirl")
        .build();
    app.connect_activate(build_ui);
    app.connect_startup(|_| {
        adw::init().unwrap();
    });
    app.run()
}

fn build_ui(app: &Application) {
    let window = GameGirlWindow::new(app);
    window.add_controller(gui::key_controller(&window));
    window.connect_destroy(|window| {
        window.save_game();
        window.imp().state.borrow().options.to_disk();
    });
    window.present();
}
