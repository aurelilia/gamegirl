use gamegirl::frontend::input::{InputAction, InputSource};
use gtk::EventControllerKey;
use window::GameGirlWindow;

use crate::state;

pub mod actions;
pub mod canvas;
pub mod input;
pub mod settings;
pub mod window;

pub fn key_controller(window: &GameGirlWindow) -> EventControllerKey {
    let core1 = window.core();
    let core2 = window.core();

    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _, _| {
        let key = key.to_upper();
        if let Some(InputAction::Button(button)) = state()
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
        if let Some(InputAction::Button(button)) = state()
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
