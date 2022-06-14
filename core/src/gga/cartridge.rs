use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Cartridge {
    #[serde(skip)]
    #[serde(default)]
    pub rom: Vec<u8>,
}

impl Cartridge {
    pub fn title(&self) -> String {
        self.read_string(0x0A0, 12)
    }

    pub fn game_code(&self) -> String {
        self.read_string(0x0AC, 4)
    }

    fn read_string(&self, base: usize, max: usize) -> String {
        let mut buf = String::new();
        for idx in 0..max {
            let ch = self.rom[base + idx] as char;
            if ch == '\0' {
                break;
            }
            buf.push(ch);
        }
        buf
    }
}
