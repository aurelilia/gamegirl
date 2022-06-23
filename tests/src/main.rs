#![feature(is_some_with)]

mod blargg;
mod gba;
mod mooneye;

use ansi_term::Colour;
use core::{ggc::GGConfig, System};
use rayon::prelude::*;
use std::{
    env, fs,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

const TIMEOUT: usize = 30;

fn main() {
    if env::args().any(|a| a == "--bench") {
        let mut gg = System::default();
        gg.load_cart(
            include_bytes!("../../bench.gb").to_vec(),
            None,
            &GGConfig::default(),
        );
        gg.skip_bootrom();
        for _ in 0..30 {
            gg.advance_delta(1.0);
        }
    } else {
        if env::args().any(|a| a == "--gg") {
            println!("Executing blargg tests");
            blargg::exec();
            blargg::exec_sound();
            println!("\nExecuting mooneye tests");
            mooneye::exec("acceptance");
            mooneye::exec("emulator-only");
        }
        println!("\nExecuting gba-tests");
        gba::exec_gba_tests();
        println!("\nExecuting FuzzARM");
        gba::exec_fuzzarm();
    }
}

pub fn run_dir<const SKIP: bool>(dir: &str, cond: fn(&System) -> ControlFlow<Status>) {
    let total = AtomicUsize::new(0);
    let success = AtomicUsize::new(0);
    run_inner::<SKIP>(
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

fn run_inner<const SKIP: bool>(
    dir: &Path,
    name: &str,
    total: &AtomicUsize,
    success: &AtomicUsize,
    cond: fn(&System) -> ControlFlow<Status>,
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
            run_inner::<SKIP>(&entry.path(), &name, total, success, cond);
        });

    entries
        .par_iter()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|s| s.to_str().unwrap() == "gb" || s.to_str().unwrap() == "gba")
        })
        .for_each(|entry| {
            let rn = Instant::now();
            total.fetch_add(1, Ordering::Relaxed);
            match run::<SKIP>(fs::read(entry.path()).unwrap(), cond) {
                Ok(_) => {
                    println!(
                        "Ran {name}/{}... {} in {}ms",
                        entry.file_name().to_str().unwrap(),
                        Colour::Green.bold().paint("SUCCESS"),
                        rn.elapsed().as_micros() as f64 / 1000.0
                    );
                    success.fetch_add(1, Ordering::Relaxed);
                }
                Err(err) => {
                    println!(
                        "Ran {name}/{}... {} in {}ms",
                        entry.file_name().to_str().unwrap(),
                        Colour::Red.bold().paint(err),
                        rn.elapsed().as_micros() as f64 / 1000.0
                    );
                }
            }
        });
}

fn run<const SKIP: bool>(
    test: Vec<u8>,
    cond: fn(&System) -> ControlFlow<Status>,
) -> Result<(), String> {
    let mut gg = System::default();
    gg.load_cart(test, None, &GGConfig::default());
    if SKIP {
        gg.skip_bootrom();
    }

    for _ in 0..TIMEOUT {
        gg.advance_delta(1.0);
        match cond(&gg) {
            ControlFlow::Break(Status::Success) => return Ok(()),
            ControlFlow::Break(Status::Fail) => return Err("FAILED".to_string()),
            ControlFlow::Break(Status::FailAt(pos)) => return Err(format!("FAILED AT {pos}")),
            _ => (),
        }
    }

    Err("TIMEOUT".to_string())
}

pub enum Status {
    Success,
    Fail,
    FailAt(String),
}
