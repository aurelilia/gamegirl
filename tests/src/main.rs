#![feature(is_some_with)]

mod blargg;
mod mooneye;

use ansi_term::Colour;
use gamegirl::system::GameGirl;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::Instant;
use rayon::prelude::*;

const TIMEOUT: usize = 30;

fn main() {
    println!("Executing blargg tests");
    blargg::exec();
    blargg::exec_sound();
    println!("Executing mooneye tests");
    mooneye::exec();
}

pub fn run_dir(dir: &str, cond: fn(&GameGirl) -> ControlFlow<bool>) {
    run_inner(&PathBuf::from("tests").join(dir), dir, cond);
}

fn run_inner(dir: &Path, name: &str, cond: fn(&GameGirl) -> ControlFlow<bool>) {
    let mut entries = dir
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());

    entries.par_iter().filter(|e| e.path().is_dir()).for_each(|entry| {
        let name = format!("{name}/{}", entry.file_name().to_str().unwrap());
        run_inner(&entry.path(), &name, cond);
    });

    entries.par_iter().filter(|e| {
        e.path()
            .extension()
            .is_some_and(|s| s.to_str().unwrap() == "gb")
    }).for_each(|entry| {
        let rn = Instant::now();
        match run(fs::read(entry.path()).unwrap(), cond) {
            Ok(_) => println!(
                "Ran {name}/{}... {} in {}ms",
                entry.file_name().to_str().unwrap(),
                Colour::Green.bold().paint("SUCCESS"),
                rn.elapsed().as_micros() as f64 / 1000.0
            ),
            Err(serial) => {
                println!(
                    "Ran {name}/{}... {} in {}ms\nOutput: {serial}",
                    entry.file_name().to_str().unwrap(),
                    Colour::Red.bold().paint("FAIL"),
                    rn.elapsed().as_micros() as f64 / 1000.0
                );
            }
        }
    });
}

fn run(test: Vec<u8>, cond: fn(&GameGirl) -> ControlFlow<bool>) -> Result<(), String> {
    let mut gg = GameGirl::with_cart(test);

    for _ in 0..TIMEOUT {
        gg.advance_delta(1.0);
        match cond(&gg) {
            ControlFlow::Break(s) if s => return Ok(()),
            ControlFlow::Break(_) => break,
            _ => (),
        }
    }

    let ret = Err(gg.debugger.serial_output.lock().unwrap().clone());
    ret
}
