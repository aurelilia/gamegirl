use std::{
    fs,
    sync::{Arc, Mutex},
};

use adw::{HeaderBar, Toast};
use gamegirl::{Core, GameCart, Storage, SystemConfig};
use gtk::{
    EventControllerKey, Label,
    gio::{Cancellable, prelude::FileExt},
    glib::object::Cast,
    prelude::{ButtonExt, WidgetExt},
};
use input::Input;

use crate::gtk;

pub mod canvas;
pub mod input;

pub fn header() -> HeaderBar {
    let header = HeaderBar::new();

    let open_button = gtk::Button::builder()
        .label("Open")
        .halign(gtk::Align::Start)
        .margin_start(10)
        .focusable(false)
        .build();
    open_button.set_parent(&header);
    open_button.connect_clicked(move |button| {
        rom_file_picker(button.parent().unwrap().downcast().unwrap())
    });

    header
}

fn rom_file_picker(header: HeaderBar) {
    let dialog = gtk::FileDialog::builder()
        .title("Open File")
        .accept_label("Open")
        .modal(true)
        .build();
    dialog.open(
        Option::<&gtk::Window>::None,
        Option::<&Cancellable>::None,
        move |file| {
            if let Some((path, rom)) = file
                .ok()
                .and_then(|f| f.path())
                .and_then(|p| fs::read(&p).ok().map(|b| (p, b)))
            {
                let title = format!("gamegirl - {}", path.file_stem().unwrap().display());
                let save = Storage::load(Some(path), "".into());

                match gamegirl::load_cart(GameCart { rom, save }, &SystemConfig::default()) {
                    Ok(sys) => {
                        *gtk().core.lock().unwrap() = sys;
                        gtk().toast.add_toast(Toast::new("Loaded ROM!"));

                        let label = Label::builder().label(title).css_classes(["title"]).build();
                        header.set_title_widget(Some(&label));
                    }
                    Err(err) => {
                        gtk()
                            .toast
                            .add_toast(Toast::new(&format!("Failed to load ROM: {}", err)));
                    }
                }
            } else {
                gtk().toast.add_toast(Toast::new("Failed to load ROM!"));
            }
        },
    );
}

pub fn key_controller() -> EventControllerKey {
    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        let input = Input::new();
        let key = key.to_upper();
        if let Some(input::InputAction::Button(button)) = input.get(input::InputSource::Key(key)) {
            gtk()
                .core
                .lock()
                .unwrap()
                .c_mut()
                .input
                .set(0, button, true);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    controller.connect_key_released(move |_, key, _, _| {
        let input = Input::new();
        let key = key.to_upper();
        if let Some(input::InputAction::Button(button)) = input.get(input::InputSource::Key(key)) {
            gtk()
                .core
                .lock()
                .unwrap()
                .c_mut()
                .input
                .set(0, button, false);
        }
    });
    controller
}
