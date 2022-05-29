mod gui;

use eframe::egui;
use gamegirl::system::GameGirl;
use std::env::args;
use std::fs;

fn main() {
    let gg = GameGirl::new(fs::read(args().nth(1).unwrap()).unwrap(), None);
    gui::start(gg);
}
