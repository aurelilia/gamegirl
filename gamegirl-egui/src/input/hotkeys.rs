// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::file_dialog;
use crate::app::App;

type HotkeyFn = fn(&mut App, bool);
pub const HOTKEYS: &[(&str, HotkeyFn)] = &[
    ("Open ROM", |a, p| {
        pressed(a, p, |app| file_dialog::open(app.message_channel.0.clone()))
    }),
    ("Reset", |a, p| {
        pressed(a, p, |app| app.core.lock().unwrap().reset())
    }),
    ("Pause", |a, p| {
        pressed(a, p, |app| {
            let mut core = app.core.lock().unwrap();
            *core.is_running() = !*core.is_running() && core.options().rom_loaded;
        })
    }),
    ("Save", |a, p| pressed(a, p, |app| app.save_game())),
    ("Fast Forward (Hold)", |app, pressed| {
        let mut core = app.core.lock().unwrap();
        if pressed {
            core.options().speed_multiplier = app.state.options.fast_forward_hold_speed;
        } else {
            core.options().speed_multiplier = 1;
        }
    }),
    ("Fast Forward (Toggle)", |a, p| {
        pressed(a, p, |app| {
            let mut core = app.core.lock().unwrap();
            app.fast_forward_toggled = !app.fast_forward_toggled;
            if app.fast_forward_toggled {
                core.options().speed_multiplier = app.state.options.fast_forward_toggle_speed;
            } else {
                core.options().speed_multiplier = 1;
            }
        });
    }),
    ("Rewind (Hold)", |app, pressed| {
        app.rewinder.rewinding = pressed;
        app.core.lock().unwrap().options().invert_audio_samples = pressed;
    }),
];

fn pressed(app: &mut App, pressed: bool, inner: fn(&mut App)) {
    if pressed {
        inner(app);
    }
}
