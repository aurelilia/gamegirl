#![feature(is_some_with)]

mod blargg;
mod mooneye;

use ansi_term::Colour;
use gamegirl::system::GameGirl;
use rayon::prelude::*;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::{env, fs};

const TIMEOUT: usize = 30;

fn main() {
    if env::args().any(|a| a == "--bench") {
        let mut gg = GameGirl::with_cart(fs::read("bench.gb").unwrap());
        for _ in 0..15 {
            gg.advance_delta(1.0);
        }
    } else {
        println!("Executing blargg tests");
        blargg::exec();
        blargg::exec_sound();
        println!("\nExecuting mooneye tests");
        mooneye::exec("acceptance");
        mooneye::exec("emulator-only");
    }
}

pub fn run_dir(dir: &str, cond: fn(&GameGirl) -> ControlFlow<bool>) {
    let total = AtomicUsize::new(0);
    let success = AtomicUsize::new(0);
    run_inner(
        &PathBuf::from("tests").join(dir),
        dir,
        &total,
        &success,
        cond,
    );
    println!(
        "{}/{} tests succeeded",
        success.load(Ordering::Relaxed),
        total.load(Ordering::Relaxed)
    );
}

fn run_inner(
    dir: &Path,
    name: &str,
    total: &AtomicUsize,
    success: &AtomicUsize,
    cond: fn(&GameGirl) -> ControlFlow<bool>,
) {
    let mut entries = dir
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| !e.file_name().to_str().unwrap().contains("disabled"))
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());

    entries
        .par_iter()
        .filter(|e| e.path().is_dir())
        .for_each(|entry| {
            let name = format!("{name}/{}", entry.file_name().to_str().unwrap());
            run_inner(&entry.path(), &name, total, success, cond);
        });

    entries
        .par_iter()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|s| s.to_str().unwrap() == "gb")
        })
        .for_each(|entry| {
            let rn = Instant::now();
            total.fetch_add(1, Ordering::Relaxed);
            match run(fs::read(entry.path()).unwrap(), cond) {
                Ok(_) => {
                    println!(
                        "Ran {name}/{}... {} in {}ms",
                        entry.file_name().to_str().unwrap(),
                        Colour::Green.bold().paint("SUCCESS"),
                        rn.elapsed().as_micros() as f64 / 1000.0
                    );
                    success.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    println!(
                        "Ran {name}/{}... {} in {}ms",
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
