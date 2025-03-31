mod config;
mod gui;

use std::{
    cell::{Cell, OnceCell, RefCell},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use adw::Application;
use config::State;
use gamegirl::Core;
use gtk::{
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{self},
    prelude::{GtkWindowExt, WidgetExt},
};
use gui::window::GameGirlWindow;

thread_local! {
    static APP: OnceCell<&'static AppState> = OnceCell::new();
}

fn state() -> &'static AppState {
    APP.with(|a| *a.get().unwrap())
}

pub struct AppState {
    core: Arc<Mutex<Box<dyn Core>>>,
    state: RefCell<State>,
    current_rom_path: RefCell<Option<PathBuf>>,
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("eu.catin.gamegirl")
        .build();
    app.connect_activate(build_ui);
    app.connect_startup(|_| {
        adw::init().unwrap();
    });
    app.connect_shutdown(|_| println!("oops"));
    app.run()
}

fn build_ui(app: &Application) {
    let (window, core) = GameGirlWindow::new(app);
    window.add_controller(gui::key_controller(&window));
    window.present();

    APP.with(|a| {
        a.set(Box::leak(Box::new(AppState {
            core,
            state: RefCell::default(),
            current_rom_path: RefCell::default(),
        })))
        .ok()
        .unwrap()
    });
}
