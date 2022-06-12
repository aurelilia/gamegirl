use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Cartridge {
    #[serde(skip)]
    #[serde(default)]
    pub rom: Vec<u8>,
}
