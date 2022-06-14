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
    pub fn save(path: Option<PathBuf>, save: GameSave) {
        let sav_path = Self::get_path(path.clone().unwrap(), "sav");
        std::fs::write(sav_path, save.ram).ok(); // TODO handle error

        if let Some(rtc) = save.rtc {
            let path = Self::get_path(path.unwrap(), "rtc");
            std::fs::write(path, format!("{}", rtc)).ok(); // TODO handle error
        }
    }

    /// Load the given cart's RAM from disk, replacing existing RAM.
    /// Path should always be Some and point to the game ROM path,
    /// since this is on native.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: Option<PathBuf>, title: String) -> Option<GameSave> {
        let sav_path = Self::get_path(path.clone().unwrap(), "sav");
        let ram = if let Ok(ram) = std::fs::read(sav_path) {
            ram
        } else {
            return None;
        };

        let path = Self::get_path(path.unwrap(), "rtc");
        let rtc = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.parse::<u64>().ok());

        Some(GameSave { ram, rtc, title })
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
    pub fn save(_path: Option<PathBuf>, save: GameSave) {
        let content = base64::encode(save.ram);
        Self::local_storage().set(&save.title, &content).ok();

        if let Some(rtc) = save.rtc {
            Self::local_storage()
                .set(&format!("{}-rtc", save.title), &format!("{}", rtc))
                .ok();
        }
    }

    /// Load the given cart's RAM from disk, replacing existing RAM.
    /// Path will always be None, since this is WASM.
    #[cfg(target_arch = "wasm32")]
    pub fn load(_path: Option<PathBuf>, title: String) -> Option<GameSave> {
        let base64 = Self::local_storage().get(&title).ok().flatten();
        let ram = if let Some(ram) = base64.and_then(|ram| base64::decode(ram).ok()) {
            ram
        } else {
            return None;
        };

        let stor = Self::local_storage()
            .get(&format!("{}-rtc", &title))
            .ok()
            .flatten();
        let rtc = stor.and_then(|s| s.parse::<u64>().ok());

        Some(GameSave { ram, rtc, title })
    }

    /// Get the browser's local storage.
    #[cfg(target_arch = "wasm32")]
    fn local_storage() -> web_sys::Storage {
        web_sys::window().unwrap().local_storage().unwrap().unwrap()
    }
}

/// Abstract game save that can be loaded by a cartridge.
pub struct GameSave {
    /// The game's RAM.
    pub ram: Vec<u8>,
    /// RTC time, for GGC games.
    pub rtc: Option<u64>,
    /// Game title.
    pub title: String,
}
