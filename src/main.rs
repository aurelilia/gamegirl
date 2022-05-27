#![feature(exclusive_range_pattern)]
#![feature(mixed_integer_ops)]

use eframe::egui;
use std::fs;
use system::GameGirl;

mod gui;
pub mod numutil;
mod system;

fn main() {
    let gg = GameGirl::new(fs::read("test.gb").unwrap());
    gui::start(gg);
}
