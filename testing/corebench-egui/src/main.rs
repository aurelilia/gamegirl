// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

mod app;
mod gui;
mod tests;
mod testsuite;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use dynacore::{common::Core, gamegirl::dummy_core, NewCoreFn};
use eframe::{egui::ViewportBuilder, emath::History, Theme};
use libloading::{Library, Symbol};
use testsuite::TestSuiteResult;

use crate::app::App;

pub struct DCore {
    c: Box<dyn Core>,
    suites: Vec<TestSuiteResult>,
    bench: History<f64>,
    bench_iso: Arc<Mutex<History<f64>>>,
    loader: NewCoreFn,
    _library: Option<Library>,
    name: String,
}

fn main() {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_transparent(true),
        default_theme: Theme::Dark,
        ..Default::default()
    };
    eframe::run_native(
        "gamegirl core workbench",
        options,
        Box::new(|ctx| App::new(ctx)),
    )
    .unwrap()
}

fn load_core(path: PathBuf) -> Result<DCore, libloading::Error> {
    unsafe {
        let lib = Library::new(&path)?;
        let fun: Symbol<NewCoreFn> = lib.get(b"new_core")?;
        Ok(DCore {
            c: dummy_core(),
            suites: vec![],
            bench: History::new(10..5000, 30.0),
            bench_iso: Arc::new(Mutex::new(History::new(10..5000, 100.0))),
            loader: *fun,
            _library: Some(lib),
            name: path.file_name().unwrap().to_string_lossy().to_string(),
        })
    }
}
