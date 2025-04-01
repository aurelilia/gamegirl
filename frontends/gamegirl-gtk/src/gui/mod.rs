use adw::subclass::prelude::ObjectSubclassIsExt;
use gamegirl::frontend::input::{InputAction, InputSource};
use gtk::EventControllerKey;
use window::GameGirlWindow;

pub mod actions;
pub mod canvas;
pub mod input;
pub mod settings;
pub mod window;

pub fn key_controller(window: &GameGirlWindow) -> EventControllerKey {
    let window1 = window.clone();
    let window2 = window.clone();
    let core1 = window.core();
    let core2 = window.core();

    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        let key = key.to_upper();
        if let Some(InputAction::Button(button)) = window1
            .imp()
            .state
            .borrow_mut()
            .options
            .input
            .key_triggered(InputSource::Key(key.into()))
        {
            core1.lock().unwrap().c_mut().input.set(0, button, true);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    controller.connect_key_released(move |_, key, _, _| {
        let key = key.to_upper();
        if let Some(InputAction::Button(button)) = window2
            .imp()
            .state
            .borrow_mut()
            .options
            .input
            .key_triggered(InputSource::Key(key.into()))
        {
            core2.lock().unwrap().c_mut().input.set(0, button, false);
        }
    });
    controller
}
