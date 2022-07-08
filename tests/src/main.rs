// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(is_some_with)]

mod gb;
mod gba;

use core::{common::SystemConfig, System};
use std::{
    env, fs,
    fs::File,
    io::BufWriter,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use ansi_term::Colour;
use png::{BitDepth, ColorType, Decoder, Encoder};
use rayon::prelude::*;
use seahorse::{App, Command, Flag, FlagType};

const TIMEOUT: usize = 30;

fn main() {
    let args = env::args().collect();
    App::new("GameGirl tests")
        .description("Automated test runner for GameGirl")
        .usage("tests [good] [bench] [--gg]")
        .flag(
            Flag::new("gg", FlagType::Bool).description("Also run GG tests (only GGA by default)"),
        )
        .command(
            Command::new("good")
                .description("Mark the given test as good by making a comparison image for it")
                .usage("tests good [path]")
                .action(|c| {
                    for path in &c.args {
                        let test_rom = fs::read(path).unwrap();
                        let img_path = format!("{}.png", path);
                        let img_path = PathBuf::from(img_path);
                        let img = run::<false, true>(test_rom, None, |_| ControlFlow::Continue(()))
                            .unwrap();
                        save_png(&img_path, img);
                    }
                }),
        )
        .command(
            Command::new("bench")
                .description("Run a benchmark ROM")
                .usage("tests bench")
                .action(|_| {
                    let mut gg = System::default();
                    gg.load_cart(
                        include_bytes!("../../bench.gb").to_vec(),
                        None,
                        &SystemConfig::default(),
                    );
                    gg.skip_bootrom();
                    for _ in 0..30 {
                        gg.advance_delta(1.0);
                    }
                }),
        )
        .action(|c| {
            if c.bool_flag("gg") {
                println!("Executing blargg tests");
                gb::blargg();
                gb::blargg_sound();
                println!("\nExecuting mooneye tests");
                gb::mooneye("acceptance");
                gb::mooneye("emulator-only");
                println!("\nExecuting acid2 tests");
                gb::acid2();
            }
            println!("\nExecuting jsmolka's gba-tests");
            gba::exec_jsmolka();
            println!("\nExecuting FuzzARM");
            gba::exec_fuzzarm();
            println!("\nExecuting ladystarbreeze's tests");
            gba::exec_ladystarbreeze();
            println!("\nExecuting destoer's tests");
            gba::exec_destoer();
        })
        .run(args);
}

pub fn run_dir<const SKIP_BOOTROM: bool, const IMG_COMPARE: bool>(
    dir: &str,
    cond: fn(&System) -> ControlFlow<Status>,
) {
    let total = AtomicUsize::new(0);
    let success = AtomicUsize::new(0);
    run_inner::<SKIP_BOOTROM, IMG_COMPARE>(
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

fn run_inner<const SKIP_BOOTROM: bool, const IMG_COMPARE: bool>(
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
            run_inner::<SKIP_BOOTROM, IMG_COMPARE>(&entry.path(), &name, total, success, cond);
        });

    entries
        .par_iter()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|s| s.to_str().unwrap() == "gb" || s.to_str().unwrap() == "gba")
        })
        .for_each(|entry| {
            let test_rom = fs::read(entry.path()).unwrap();
            let img_path = format!("{}.png", entry.path().to_str().unwrap());
            let img_path = PathBuf::from(img_path);
            let img = if IMG_COMPARE {
                load_png(&img_path)
            } else {
                None
            };

            let rn = Instant::now();
            total.fetch_add(1, Ordering::Relaxed);
            match run::<SKIP_BOOTROM, false>(test_rom, img, cond) {
                Ok(frame) => {
                    println!(
                        "Ran {name}/{}... {} in {}ms",
                        entry.file_name().to_str().unwrap(),
                        Colour::Green.bold().paint("SUCCESS"),
                        rn.elapsed().as_micros() as f64 / 1000.0
                    );
                    success.fetch_add(1, Ordering::Relaxed);
                    if IMG_COMPARE {
                        save_png(&img_path, frame);
                    }
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

fn run<const SKIP_BOOTROM: bool, const TIMEOUT_GOOD: bool>(
    test: Vec<u8>,
    image: Option<Vec<core::Colour>>,
    cond: fn(&System) -> ControlFlow<Status>,
) -> Result<Vec<core::Colour>, String> {
    let mut gg = System::default();
    gg.load_cart(test, None, &SystemConfig::default());
    if SKIP_BOOTROM {
        gg.skip_bootrom();
    }

    for i in 0..=TIMEOUT {
        gg.advance_delta(1.0);
        let frame = gg.last_frame().unwrap();
        if let Some(img) = &image {
            if *img == frame {
                return Ok(frame);
            }
        }
        match cond(&gg) {
            ControlFlow::Break(Status::Success) => return Ok(frame),
            ControlFlow::Break(Status::Fail) => return Err("FAILED".to_string()),
            ControlFlow::Break(Status::FailAt(pos)) => return Err(format!("FAILED AT {pos}")),
            _ => (),
        }

        if i == TIMEOUT && TIMEOUT_GOOD {
            return Ok(frame);
        }
    }

    Err("TIMEOUT".to_string())
}

pub enum Status {
    Success,
    Fail,
    FailAt(String),
}

fn load_png(path: &Path) -> Option<Vec<core::Colour>> {
    let raw_img = File::open(path).ok();
    raw_img.map(|i| {
        let dec = Decoder::new(i);
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(info.color_type, ColorType::Rgba);
        assert_eq!(info.bit_depth, BitDepth::Eight);
        buf.truncate(info.buffer_size());
        let mut out = Vec::new();
        for en in buf.chunks(4) {
            out.push([en[0], en[1], en[2], en[3]]);
        }
        out
    })
}

fn save_png(path: &Path, png: Vec<core::Colour>) {
    let (w, h) = match png.len() {
        23040 => (160, 144),
        38400 => (240, 160),
        _ => panic!("Unknown screen size"),
    };

    let file = File::create(path).unwrap();
    let mut data = Vec::new();
    for col in png {
        for byte in col {
            data.push(byte);
        }
    }

    let writer = BufWriter::new(file);
    let mut enc = Encoder::new(writer, w, h);
    enc.set_color(ColorType::Rgba);
    enc.set_depth(BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
}
