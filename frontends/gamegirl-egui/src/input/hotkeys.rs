// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use super::file_dialog;
use crate::app::App;

type HotkeyFn = fn(&mut App, bool);
pub const HOTKEYS: &[(&str, HotkeyFn)] = &[
    ("Open ROM", |a, p| {
        pressed(a, p, |app| {
            file_dialog::open_rom(app.message_channel.0.clone())
        })
    }),
    ("Reset", |a, p| pressed(a, p, App::reset)),
    ("Pause", |a, p| pressed(a, p, App::pause)),
    ("Save", |a, p| pressed(a, p, |app| app.save_game())),
    ("Fast Forward (Hold)", |app, pressed| {
        let mut core = app.core.lock().unwrap();
        let c = core.c_mut();
        if pressed {
            c.options.speed_multiplier = app.state.options.rewinder.ff_hold_speed;
        } else {
            c.options.speed_multiplier = 1;
        }
        c.video_buffer.frameskip = c.options.speed_multiplier - 1;
    }),
    ("Fast Forward (Toggle)", |a, p| {
        pressed(a, p, |app| {
            let mut core = app.core.lock().unwrap();
            let c = core.c_mut();

            app.fast_forward_toggled = !app.fast_forward_toggled;
            if app.fast_forward_toggled {
                c.options.speed_multiplier = app.state.options.rewinder.ff_toggle_speed;
            } else {
                c.options.speed_multiplier = 1;
            }
            c.video_buffer.frameskip = c.options.speed_multiplier - 1;
        });
    }),
    ("Rewind (Hold)", |app, pressed| {
        app.rewinder.rewinding = pressed;
        app.core
            .lock()
            .unwrap()
            .c_mut()
            .options
            .invert_audio_samples = pressed;
    }),
];

fn pressed(app: &mut App, pressed: bool, inner: fn(&mut App)) {
    if pressed {
        inner(app);
    }
}
