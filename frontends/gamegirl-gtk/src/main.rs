mod gui;

use std::{
    cell::OnceCell,
    sync::{Arc, Mutex},
};

use adw::{Application, ToastOverlay};
use gamegirl::Core;
use gtk::{
    ApplicationWindow,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{self},
    prelude::{GtkWindowExt, WidgetExt},
};
use gui::canvas;

thread_local! {
    static APP: OnceCell<&'static App> = OnceCell::new();
}

fn gtk() -> &'static App {
    APP.with(|a| *a.get().unwrap())
}

pub struct App {
    core: Arc<Mutex<Box<dyn Core>>>,
    window: ApplicationWindow,
    toast: ToastOverlay,
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("eu.catin.gamegirl")
        .build();
    app.connect_activate(build_ui);
    app.connect_shutdown(|_| println!("oops"));
    app.run()
}

fn build_ui(app: &Application) {
    let (pict, core) = canvas::get();
    let toast = ToastOverlay::new();
    toast.set_child(Some(&pict));
    let header = gui::header();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("gamegirl")
        .child(&toast)
        .build();
    window.set_titlebar(Some(&header));
    window.add_controller(gui::key_controller());
    window.present();

    APP.with(|a| {
        a.set(Box::leak(Box::new(App {
            core,
            window,
            toast,
        })))
        .ok()
        .unwrap()
    });
}
