#![feature(exclusive_range_pattern)]

use system::GameGirl;

pub mod numutil;
mod system;

fn main() {
    let mut gg = GameGirl::default();
    gg.advance();
}
