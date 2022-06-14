use crate::{
    gga::Access::{self, *},
    numutil::{NumExt, U16Ext},
};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct Cartridge {
    #[serde(skip)]
    #[serde(default)]
    pub rom: Vec<u8>,

    /// WAITCNT register
    pub waitcnt: u16,
}

impl Cartridge {
    pub fn title(&self) -> String {
        self.read_string(0x0A0, 12)
    }

    pub fn game_code(&self) -> String {
        self.read_string(0x0AC, 4)
    }

    const WS_NONSEQ: [u16; 4] = [4, 3, 2, 8];

    pub(super) fn wait_time(&self, addr: u32, ty: Access) -> usize {
        match (addr, ty) {
            (0x0800_0000..=0x09FF_FFFF, Seq) => 2 - self.waitcnt.bit(4),
            (0x0800_0000..=0x09FF_FFFF, NonSeq) => Self::WS_NONSEQ[self.waitcnt.bits(2, 2).us()],

            (0x0A00_0000..=0x0BFF_FFFF, Seq) => 4 - (self.waitcnt.bit(4) * 3),
            (0x0A00_0000..=0x0BFF_FFFF, NonSeq) => Self::WS_NONSEQ[self.waitcnt.bits(5, 2).us()],

            (0x0C00_0000..=0x0DFF_FFFF, Seq) => 8 - (self.waitcnt.bit(4) * 7),
            (0x0C00_0000..=0x0DFF_FFFF, NonSeq) => Self::WS_NONSEQ[self.waitcnt.bits(8, 2).us()],

            _ => 1,
        }
        .us()
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
