use crate::numutil::NumExt;
use crate::system::io::timer::Timer;
use crate::GameGirl;

pub mod addr;
mod apu;
mod cartridge;
mod dma;
mod joypad;
mod ppu;
mod timer;

pub struct Mmu {
    pub vram: [u8; 2 * 8192],
    pub vram_bank: u8,
    pub wram: [u8; 2 * 8192],
    pub wram_bank: u8,
    pub oam: [u8; 160],
    pub zram: [u8; 127],
    pub in_bootrom: bool,

    pub timer: Timer,
}

impl Mmu {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        todo!()
    }

    pub fn read(&self, _addr: u16) -> u8 {
        todo!()
    }

    pub fn write(&mut self, _addr: u16, _value: u8) {
        todo!()
    }

    pub fn read16(&self, addr: u16) -> u16 {
        let low = self.read(addr);
        let high = self.read(addr + 1);
        (high.u16() << 8) | low.u16()
    }

    pub fn write16(&mut self, addr: u16, value: u16) {
        self.write(addr, value.u8());
        self.write(addr + 1, (value << 8).u8());
    }
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            vram: [0; 16384],
            vram_bank: 0,
            wram: [0; 16384],
            wram_bank: 0,
            oam: [0; 160],
            zram: [0; 127],
            in_bootrom: true,

            timer: Timer::default(),
        }
    }
}

impl GameGirl {
    fn timer(&mut self) -> &mut Timer {
        &mut self.mmu.timer
    }
}
