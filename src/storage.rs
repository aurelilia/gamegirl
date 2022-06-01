use crate::system::io::cartridge::Cartridge;
use std::path::PathBuf;

pub struct Storage;

impl Storage {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(path: Option<PathBuf>, cart: &Cartridge) {
        let path = Self::get_sav_path(path.unwrap());
        std::fs::write(path, &cart.ram()).ok(); // TODO handle error
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: Option<PathBuf>, cart: &mut Cartridge) {
        let path = Self::get_sav_path(path.unwrap());
        if let Ok(ram) = std::fs::read(path) {
            cart.load_ram(ram);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_sav_path(mut path: PathBuf) -> PathBuf {
        let base = path.file_stem().unwrap().to_str().unwrap();
        let name = format!("{base}.sav");
        path.pop();
        path.push(name);
        path
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(_path: Option<PathBuf>, cart: &Cartridge) {
        let content = base64::encode(cart.ram());
        Self::local_storage().set(&cart.title(true), &content).ok();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load(_path: Option<PathBuf>, cart: &mut Cartridge) {
        let base64 = Self::local_storage().get(&cart.title(true)).ok().flatten();
        if let Some(ram) = base64.and_then(|ram| base64::decode(ram).ok()) {
            cart.load_ram(ram);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn local_storage() -> web_sys::Storage {
        web_sys::window().unwrap().local_storage().unwrap().unwrap()
    }
}
