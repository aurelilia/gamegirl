use std::{
    borrow::Cow,
    fs::{self, DirEntry, File},
    io::Read,
    panic,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clap::{Parser, Subcommand};
use gamegirl::{
    common::common::{
        input::Button,
        options::{ConsoleBios, SystemConfig},
    },
    Core, GameCart,
};
use indicatif::{MultiProgress, ProgressBar};
use png::{BitDepth, ColorType, Encoder};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a single game for a little while
    RunGame {
        /// Path of the game to run
        path: PathBuf,
        /// Time in seconds to run for
        time: u32,
    },
    /// Take screenshots of all games given
    Screenshot {
        /// Only run games with the given keywords in the name
        #[arg(short, long)]
        include: Vec<String>,
        /// Exclude games with the given keywords in the name
        #[arg(short, long)]
        exclude: Vec<String>,
        /// Assume games to be zipped, unzip them before running
        #[arg(short, long)]
        zipped: bool,
        /// Run only on a single thread. Useful for debugging emulator crashes
        #[arg(short, long)]
        single: bool,
        /// Do not run games which already have a screenshot
        #[arg(short, long)]
        only_new: bool,

        /// BIOS file
        bios: PathBuf,

        /// Directory to run roms from
        rom_path: PathBuf,

        /// Directory to place screenshots in
        output_path: PathBuf,
    },
}

fn main() {
    let args = Args::parse();
    match args.command {
        Commands::RunGame { path, time } => {
            let mut core = gamegirl::load_cart(
                GameCart {
                    rom: fs::read(path).unwrap(),
                    save: None,
                },
                &SystemConfig::default(),
            )
            .unwrap();
            core.skip_bootrom();
            for _ in 0..(time / 10) {
                core.advance_delta(0.1);
            }
        }

        Commands::Screenshot {
            include,
            exclude,
            zipped,
            single,
            only_new,
            bios,
            rom_path,
            output_path,
        } => screenshot(
            include,
            exclude,
            zipped,
            single,
            only_new,
            bios,
            rom_path,
            output_path,
        ),
    }
}

fn screenshot(
    include: Vec<String>,
    exclude: Vec<String>,
    zipped: bool,
    single: bool,
    only_new: bool,
    bios: PathBuf,
    rom_path: PathBuf,
    output_path: PathBuf,
) {
    let inc = include
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<String>>();
    let exc = exclude
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<String>>();

    let game_paths: Vec<(DirEntry, String)> = rom_path
        .read_dir()
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|rom| {
            let name = rom.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            let not_filtered = inc.iter().all(|keyword| lower.contains(keyword))
                && exc.iter().all(|keyword| !lower.contains(keyword));
            let exists = only_new && output_path.join(format!("{}.astart.png", name)).exists();
            (not_filtered && !exists).then_some((rom, name))
        })
        .collect();
    println!("Collected {} games", game_paths.len());

    let mp = MultiProgress::new();
    let total_bar = mp.add(ProgressBar::new(game_paths.len() as u64));
    total_bar.enable_steady_tick(Duration::from_millis(100));
    total_bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar} {pos}/{len}, {percent}% (ETA: {eta_precise})")
            .unwrap(),
    );

    let config = SystemConfig {
        threaded_ppu: false,
        bioses: vec![ConsoleBios {
            console_id: "agb".into(),
            console_name: "AGB".into(),
            bios: Some(fs::read(&bios).unwrap()),
        }],
        ..SystemConfig::default()
    };

    if single {
        game_paths.into_iter().for_each(|(file, name)| {
            run_game_safe(&mp, name, file, zipped, &output_path, &config, &total_bar);
        });
    } else {
        game_paths.into_par_iter().for_each(|(file, name)| {
            run_game_safe(&mp, name, file, zipped, &output_path, &config, &total_bar);
        });
    }
}

fn run_game_safe(
    mp: &MultiProgress,
    name: String,
    file: DirEntry,
    zipped: bool,
    output_path: &PathBuf,
    config: &SystemConfig,
    total_bar: &ProgressBar,
) {
    let bar = mp.insert_from_back(1, ProgressBar::new_spinner());
    let result = panic::catch_unwind(|| {
        run_game(
            mp,
            &bar,
            &name,
            file,
            zipped,
            output_path,
            config,
            total_bar,
        );
    });
    if let Err(e) = result {
        bar.finish_with_message(Cow::Owned(format!("Panic running {name}: {:?}", e)));
        total_bar.inc(1);
    }
}

fn run_game(
    mp: &MultiProgress,
    bar: &ProgressBar,
    name: &str,
    file: DirEntry,
    zipped: bool,
    output_path: &PathBuf,
    config: &SystemConfig,
    total_bar: &ProgressBar,
) {
    bar.set_message(Cow::Owned(name.to_string()));
    let path = file.path();

    let rom = if zipped {
        let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let mut rom = Vec::new();
        archive.by_index(0).unwrap().read_to_end(&mut rom).unwrap();
        rom
    } else {
        fs::read(&path).unwrap()
    };
    let mut core = gamegirl::load_cart(GameCart { rom, save: None }, config).unwrap();

    for _ in 0..80 {
        let time = Instant::now();
        core.advance_delta(0.5);
        if time.elapsed() > Duration::from_secs(1) {
            // This game is too slow, skip it
            // Likely emulator bug
            bar.finish_with_message(Cow::Owned(format!("Skipping {name}: Ran too slow!")));
            total_bar.inc(1);
            return;
        }
        bar.tick();
    }
    write_png(&output_path, &mut core, name, "noinput");

    for _ in 0..15 {
        core.advance_delta(1.5);
        core.c_mut().input.set(0, Button::Start, true);
        core.advance_delta(0.5);
        core.c_mut().input.set(0, Button::Start, false);
        core.advance_delta(0.5);
        core.c_mut().input.set(0, Button::A, true);
        core.advance_delta(0.5);
        core.c_mut().input.set(0, Button::A, false);
        bar.tick();
    }
    write_png(&output_path, &mut core, name, "astart");

    mp.remove(bar);
    bar.abandon();
    total_bar.inc(1);
}

fn write_png(base_path: &Path, core: &mut Box<dyn Core>, name: &str, ext: &str) {
    let Some(image) = core.c_mut().video_buffer.pop_recent() else {
        return;
    };
    let size = core.screen_size();

    let mut encoder = Encoder::new(
        File::create(base_path.join(format!("{name}.{ext}.png"))).unwrap(),
        size[0] as u32,
        size[1] as u32,
    );
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(
            &image
                .into_iter()
                .flat_map(|i| i.into_iter())
                .collect::<Vec<_>>(),
        )
        .unwrap();
}
