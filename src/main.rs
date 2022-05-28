#![feature(exclusive_range_pattern)]
#![feature(mixed_integer_ops)]
#![allow(unused)]

use eframe::egui;
use std::env::args;
use std::fs;
use system::GameGirl;

mod gui;
pub mod numutil;
mod system;

fn main() {
    let gg = GameGirl::new(fs::read(args().skip(1).next().unwrap()).unwrap());
    gui::start(gg);
}
