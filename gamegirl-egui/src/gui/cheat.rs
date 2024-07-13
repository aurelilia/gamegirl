use std::cmp::Ordering;

use common::Width;
use egui::{Button, Checkbox, Color32, Context, Layout, Separator, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};

use crate::App;

#[derive(Default)]
pub struct CheatEngineState {
    pub search: String,
    pub results: Vec<SearchResult>,
    pub searches: Vec<u32>,
    pub is_first: bool,
    pub typ: Width,
}

#[derive(Clone)]
pub struct SearchResult {
    pub address: u32,
    pub value: u32,
    pub value_text: String,
    pub sticky: bool,
}

pub fn ui(app: &mut App, _ctx: &Context, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Memory Search");
        ui.add_space(20.0);
        if ui.button("Reset Search").clicked() {
            app.cheat.results.retain(|x| x.sticky);
            app.cheat.is_first = true;
        }
    });

    ui.add(Separator::default().spacing(10.));

    TableBuilder::new(ui)
        .striped(true)
        .column(Column::exact(85.0))
        .column(Column::initial(200.0).at_least(200.0))
        .column(Column::exact(35.0))
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.centered_and_justified(|ui| ui.strong("Address"));
            });
            header.col(|ui| {
                ui.centered_and_justified(|ui| ui.strong("Value"));
            });
            header.col(|ui| {
                ui.centered_and_justified(|ui| ui.strong("Stick").on_hover_text(
                    "Keep this address in the table, even if it's not found in the next search / the search is reset.",
                ));
            });
        })
        .body(|body| {
            body.rows(20.0, app.cheat.results.len(), |mut row| {
                let entry = &mut app.cheat.results[row.index()];
                row.col(|ui| {
                    ui.centered_and_justified(|ui| ui.monospace(format!("0x{:08X}", entry.address)));
                });
                row.col(|ui| {
                    ui.centered_and_justified(|ui| {
                        if ui
                            .add(TextEdit::singleline(&mut entry.value_text).desired_width(50.0))
                            .changed()
                        {
                            if let Ok(value) = entry.value_text.parse::<u32>() {
                                entry.value = value;
                                app.core.lock().unwrap().set_memory(
                                    entry.address,
                                    value,
                                    app.cheat.typ,
                                );
                            }
                        }
                    });
                });
                row.col(|ui| {
                    ui.centered_and_justified(|ui| {
                        ui.add(Checkbox::without_text(&mut entry.sticky));
                    });
                });
            });
        });

    ui.add(Separator::default().spacing(10.));

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let search = app.cheat.search.parse::<u32>();
            let edit = TextEdit::singleline(&mut app.cheat.search).desired_width(50.0);
            let edit = if search.is_ok() {
                edit
            } else {
                edit.text_color(Color32::RED)
            };
            ui.add(edit);

            let button = Button::new("Search/Next").min_size([50.0, 0.0].into());
            let button = if search.is_ok() {
                button
            } else {
                button.fill(Color32::GRAY)
            };

            if ui.add(button).clicked() {
                let Ok(search) = search else {
                    return;
                };
                let new =
                    app.core
                        .lock()
                        .unwrap()
                        .search_memory(search, app.cheat.typ, Ordering::Equal);
                if app.cheat.is_first {
                    let mut new = new
                        .into_iter()
                        .filter(|addr| !app.cheat.results.iter().any(|r| r.address == *addr))
                        .map(|address| SearchResult {
                            address,
                            value: search,
                            value_text: format!("{}", search),
                            sticky: false,
                        })
                        .collect::<Vec<_>>();
                    app.cheat.results.append(&mut new);
                    app.cheat.is_first = false;
                } else {
                    app.cheat
                        .results
                        .retain(|x| x.sticky || new.contains(&x.address));
                    app.cheat.results.iter_mut().for_each(|x| {
                        x.value = search;
                        x.value_text = format!("{}", search);
                    });
                }
            }
        });

        ui.add(Separator::default().spacing(10.));
        ui.vertical(|ui| {
            ui.radio_value(&mut app.cheat.typ, Width::Byte, "Byte / 8-bit");
            ui.radio_value(&mut app.cheat.typ, Width::Halfword, "Halfword / 16-bit");
            ui.radio_value(&mut app.cheat.typ, Width::Word, "Word / 32-bit");
        });
    });
}
