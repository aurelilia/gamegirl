// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use eframe::Theme;
use gamegirl_egui::App;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use eframe::egui::ViewportBuilder;

    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_transparent(true),
        default_theme: Theme::Dark,
        ..Default::default()
    };
    eframe::run_native("gamegirl", options, Box::new(|ctx| App::new(ctx))).unwrap()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    wasm_bindgen_futures::spawn_local(async {
        let options = eframe::WebOptions {
            default_theme: Theme::Dark,
            ..Default::default()
        };
        eframe::WebRunner::new()
            .start("the_canvas_id", options, Box::new(|ctx| App::new(ctx)))
            .await
            .unwrap();
    });
}
