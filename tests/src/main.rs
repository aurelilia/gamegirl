#![feature(is_some_with)]

mod blargg;

use ansi_term::Colour;
use gamegirl::system::debugger::Debugger;
use gamegirl::system::GameGirl;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

const TIMEOUT: usize = 60;

fn main() {
    println!("Executing blargg tests");
    blargg::exec();
}

pub fn run_dir(dir: &str, cond: fn(&GameGirl) -> ControlFlow<bool>) {
    run_inner(&PathBuf::from("").join(dir), dir, cond);
}

fn run_inner(dir: &Path, name: &str, cond: fn(&GameGirl) -> ControlFlow<bool>) {
    let mut entries = dir
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries.iter().filter(|e| e.path().is_dir()) {
        let name = format!("{name}/{}", entry.file_name().to_str().unwrap());
        run_inner(&entry.path(), &name, cond);
    }

    for entry in entries.iter().filter(|e| {
        e.path()
            .extension()
            .is_some_and(|s| s.to_str().unwrap() == "gb")
    }) {
        print!("Running {name}/{}... ", entry.file_name().to_str().unwrap());
        let rn = Instant::now();
        match run(fs::read(entry.path()).unwrap(), cond) {
            Ok(_) => println!(
                "{} in {}ms",
                Colour::Green.bold().paint("SUCCESS"),
                rn.elapsed().as_micros() as f64 / 1000.0
            ),
            Err(serial) => {
                println!(
                    "{} in {}ms",
                    Colour::Red.bold().paint("FAIL"),
                    rn.elapsed().as_micros() as f64 / 1000.0
                );
                println!("Output: {serial}");
            }
        }
    }
}

fn run(test: Vec<u8>, cond: fn(&GameGirl) -> ControlFlow<bool>) -> Result<(), String> {
    let dbg = Arc::new(RwLock::new(Debugger::default()));
    let mut gg = GameGirl::with_cart(test, Some(dbg.clone()));

    for _ in 0..TIMEOUT {
        gg.advance_delta(1.0);
        match cond(&gg) {
            ControlFlow::Break(s) if s => return Ok(()),
            _ => (),
        }
    }

    let ret = Err(dbg.read().unwrap().serial_output.clone());
    ret
}
