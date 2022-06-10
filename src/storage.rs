use crate::system::io::cartridge::{Cartridge, MBCKind};
use std::path::PathBuf;

/// Empty struct holding methods used for interacting with the file system,
/// for storing game save data / cartridge RAM.
/// On native, will load/store `.sav` files next to game ROM files.
/// On WASM, will load/store into browser local storage.
pub struct Storage;

impl Storage {
    /// Save the given cart's RAM to disk.
    /// Path should always be Some and point to the game ROM path,
    /// since this is on native.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(path: Option<PathBuf>, cart: &Cartridge) {
        let sav_path = Self::get_path(path.clone().unwrap(), "sav");
        std::fs::write(sav_path, &cart.ram()).ok(); // TODO handle error

        if let MBCKind::MBC3RTC { rtc, .. } = &cart.kind {
            let path = Self::get_path(path.unwrap(), "rtc");
            std::fs::write(path, format!("{}", rtc.start)).ok(); // TODO handle error
        }
    }

    /// Load the given cart's RAM from disk, replacing existing RAM.
    /// Path should always be Some and point to the game ROM path,
    /// since this is on native.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: Option<PathBuf>, cart: &mut Cartridge) {
        let sav_path = Self::get_path(path.clone().unwrap(), "sav");
        if let Ok(ram) = std::fs::read(sav_path) {
            cart.load_ram(ram);
        }

        if let MBCKind::MBC3RTC { rtc, .. } = &mut cart.kind {
            let path = Self::get_path(path.unwrap(), "rtc");
            if let Some(val) = std::fs::read_to_string(path)
                .ok()
                .and_then(|s| u64::from_str_radix(&s, 10).ok())
            {
                rtc.start = val;
            }
        }
    }

    /// "hello/my/rom.gb" -> "hello/my/rom.$ext"
    #[cfg(not(target_arch = "wasm32"))]
    fn get_path(mut path: PathBuf, ext: &str) -> PathBuf {
        let base = path.file_stem().unwrap().to_str().unwrap();
        let name = format!("{base}.{ext}");
        path.pop();
        path.push(name);
        path
    }

    /// Save the given cart's RAM to local storage.
    /// Path will always be None, since this is WASM.
    #[cfg(target_arch = "wasm32")]
    pub fn save(_path: Option<PathBuf>, cart: &Cartridge) {
        let content = base64::encode(cart.ram());
        Self::local_storage().set(&cart.title(true), &content).ok();

        if let MBCKind::MBC3RTC { rtc, .. } = &cart.kind {
            Self::local_storage()
                .set(
                    &format!("{}-rtc", cart.title(true)),
                    &format!("{}", rtc.start),
                )
                .ok();
        }
    }

    /// Load the given cart's RAM from disk, replacing existing RAM.
    /// Path will always be None, since this is WASM.
    #[cfg(target_arch = "wasm32")]
    pub fn load(_path: Option<PathBuf>, cart: &mut Cartridge) {
        let title = cart.title(true);
        let base64 = Self::local_storage().get(&title).ok().flatten();
        if let Some(ram) = base64.and_then(|ram| base64::decode(ram).ok()) {
            cart.load_ram(ram);
        }

        if let MBCKind::MBC3RTC { rtc, .. } = &mut cart.kind {
            let stor = Self::local_storage()
                .get(&format!("{}-rtc", title))
                .ok()
                .flatten();
            if let Some(val) = stor.and_then(|s| u64::from_str_radix(&s, 10).ok()) {
                rtc.start = val;
            }
        }
    }

    /// Get the browser's local storage.
    #[cfg(target_arch = "wasm32")]
    fn local_storage() -> web_sys::Storage {
        web_sys::window().unwrap().local_storage().unwrap().unwrap()
    }
}
