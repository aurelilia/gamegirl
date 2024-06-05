// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{sync::Arc, thread, time::Instant};

use eframe::egui::{Context, Ui};
use egui_plot::{Legend, Line, Plot, PlotPoints};

use crate::{app::App, tests::SUITES};

pub(super) fn suites(app: &mut App, _ctx: &Context, ui: &mut Ui) {
    ui.label("Add suites:");
    for suite in SUITES {
        if ui.button(suite.0).clicked() {
            app.suites.push(Arc::new(suite.1()));
            app.update_test_suites();
        }
    }

    ui.separator();
    ui.label("Currently loaded suites:");
    for suite in &app.suites {
        ui.horizontal(|ui| {
            ui.label(&suite.name);
        });
    }
}

pub(super) fn bench(app: &mut App, _ctx: &Context, ui: &mut Ui) {
    if ui.button("Start isolated benchmark").clicked() {
        let cores = app
            .cores
            .iter()
            .map(|c| {
                c.bench_iso.lock().unwrap().clear();
                ((c.loader)(app.rom.clone().unwrap()), c.bench_iso.clone())
            })
            .collect::<Vec<_>>();
        thread::spawn(|| {
            for (mut core, bench) in cores {
                for time in 0..500 {
                    let delta = time as f64 / 5.0;
                    let time = Instant::now();
                    core.advance_delta(0.1);
                    let elapsed = time.elapsed().as_micros() as f64;
                    bench.lock().unwrap().add(delta, elapsed / 1000.0);
                }
            }
        });
    }

    ui.checkbox(&mut app.bench_iso, "Graph: Show Isolated Benchmark");

    if app.bench_iso {
        Plot::new("benchmark")
            .legend(Legend::default())
            .allow_scroll(false)
            .allow_drag(false)
            .include_x(100.0)
            .x_axis_label("Emulated Time")
            .y_axis_label("Time to emulate 0.2s in ms")
            .show(ui, |ui| {
                for core in app.cores.iter() {
                    ui.line(
                        Line::new(PlotPoints::from_iter(
                            core.bench_iso.lock().unwrap().iter().map(|(t, s)| [t, s]),
                        ))
                        .name(&core.name),
                    );
                }
            });
    } else {
        Plot::new("benchmark")
            .legend(Legend::default())
            .allow_scroll(false)
            .allow_drag(false)
            .include_x(30.0)
            .x_axis_label("Real Time")
            .y_axis_label("Time to emulate 0.2s in ms")
            .show(ui, |ui| {
                for core in app.cores.iter() {
                    ui.line(
                        Line::new(PlotPoints::from_iter(
                            core.bench.iter().map(|(t, s)| [t, s]),
                        ))
                        .name(&core.name),
                    );
                }
            });
    }
}
