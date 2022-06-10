use crate::common::EmulateOptions;
use crate::gga::audio::{APU_REG_END, APU_REG_START};
use crate::gga::cpu::{CpuMode, FiqReg, ModeReg, Regs};
use crate::gga::dma::{Dma, DMA_END, DMA_START, DMA_WIDTH};
use crate::gga::graphics::{
    OAM_END, OAM_START, PALETTE_END, PALETTE_START, PPU_REG_END, PPU_REG_START, VRAM_END,
    VRAM_START,
};
use crate::gga::input::{Input, INPUT_END, INPUT_START};
use crate::gga::memory::{
    BIOS_END, BIOS_START, KB, WRAM1_END, WRAM1_START, WRAM2_END, WRAM2_START,
};
use crate::gga::timer::{Timer, TIMER_END, TIMER_START, TIMER_WIDTH};
use crate::ggc::GGConfig;
use crate::numutil::{hword, word, NumExt, U16Ext, U32Ext};
use audio::Apu;
use cartridge::Cartridge;
use cpu::Cpu;
use graphics::Ppu;
use memory::Memory;

mod audio;
mod cartridge;
mod cpu;
mod dma;
mod graphics;
mod input;
mod memory;
mod timer;

/// Console struct representing a GGA. Contains all state and is generally used for system emulation.
/// Use [GameGirlAdv::step] to advance the emulation.
pub struct GameGirlAdv {
    pub cpu: Cpu,
    pub memory: Memory,
    pub gpu: Ppu,
    pub apu: Apu,
    pub dma: [Dma; 4],
    pub timers: [Timer; 4],
    pub input: Input,
    pub cart: Cartridge,

    pub options: EmulateOptions,
    pub config: GGConfig,
}

impl GameGirlAdv {
    /// Step forward the emulated console including all subsystems.
    pub fn step(&mut self) {
        todo!()
    }

    /// Read a byte from the bus. Does no timing-related things; simply fetches the value.
    fn read_byte(&self, addr: u32) -> u8 {
        let addr = addr.us();
        match addr {
            BIOS_START..=BIOS_END => self.memory.bios[addr - BIOS_START],
            WRAM1_START..=WRAM1_END => self.memory.wram1[addr - WRAM1_START],
            WRAM2_START..=WRAM2_END => self.memory.wram2[addr - WRAM2_START],

            PPU_REG_START..=PPU_REG_END => self.gpu.regs[addr - PPU_REG_START],
            PALETTE_START..=PALETTE_END => self.gpu.palette[addr - PALETTE_START],
            VRAM_START..=VRAM_END => self.gpu.vram[addr - VRAM_START],
            OAM_START..=OAM_END => self.gpu.oam[addr - OAM_START],

            APU_REG_START..=APU_REG_END => self.apu.regs[addr - APU_REG_START],
            DMA_START..=DMA_END => {
                let dma_idx = (addr - DMA_START) / DMA_WIDTH;
                self.dma[dma_idx].regs[addr - DMA_START - (dma_idx * DMA_WIDTH)]
            }
            TIMER_START..=TIMER_END => {
                let timer_idx = (addr - TIMER_START) / TIMER_WIDTH;
                self.timers[timer_idx].regs[addr - TIMER_START - (timer_idx * TIMER_WIDTH)]
            }
            INPUT_START..=INPUT_END => self.input.regs[addr - INPUT_START],

            0x04000200 => self.cpu.ie.low(),
            0x04000201 => self.cpu.ie.high(),
            0x04000202 => self.cpu.if_.low(),
            0x04000203 => self.cpu.if_.high(),
            0x04000208 => self.cpu.ime as u8,
            0x04000209..=0x0400020B => 0, // High unused bits of IME

            _ => 0xFF,
        }
    }

    /// Read a half-word from the bus (LE). Does no timing-related things; simply fetches the value.
    /// Also does not handle unaligned reads (yet)
    fn read_hword(&self, addr: u32) -> u16 {
        hword(self.read_byte(addr), self.read_byte(addr + 1))
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply fetches the value.
    /// Also does not handle unaligned reads (yet)
    fn read_word(&self, addr: u32) -> u32 {
        word(self.read_hword(addr), self.read_hword(addr + 2))
    }

    /// Write a byte to the bus. Does no timing-related things; simply sets the value.
    fn write_byte(&mut self, addr: u32, value: u8) {
        let addr = addr.us();
        match addr {
            BIOS_START..=BIOS_END => self.memory.bios[addr - BIOS_START] = value,
            WRAM1_START..=WRAM1_END => self.memory.wram1[addr - WRAM1_START] = value,
            WRAM2_START..=WRAM2_END => self.memory.wram2[addr - WRAM2_START] = value,

            PPU_REG_START..=PPU_REG_END => self.gpu.regs[addr - PPU_REG_START] = value,
            PALETTE_START..=PALETTE_END => self.gpu.palette[addr - PALETTE_START] = value,
            VRAM_START..=VRAM_END => self.gpu.vram[addr - VRAM_START] = value,
            OAM_START..=OAM_END => self.gpu.oam[addr - OAM_START] = value,

            APU_REG_START..=APU_REG_END => self.apu.regs[addr - APU_REG_START] = value,
            DMA_START..=DMA_END => {
                let dma_idx = (addr - DMA_START) / DMA_WIDTH;
                self.dma[dma_idx].regs[addr - DMA_START - (dma_idx * DMA_WIDTH)] = value;
            }
            TIMER_START..=TIMER_END => {
                let timer_idx = (addr - TIMER_START) / TIMER_WIDTH;
                self.timers[timer_idx].regs[addr - TIMER_START - (timer_idx * TIMER_WIDTH)] = value;
            }
            INPUT_START..=INPUT_END => self.input.regs[addr - INPUT_START] = value,

            0x04000201 => self.cpu.ie = self.cpu.ie.set_low(value),
            0x04000202 => self.cpu.if_ = self.cpu.if_.set_high(value),
            0x04000203 => self.cpu.if_ = self.cpu.if_.set_low(value),
            0x04000200 => self.cpu.ie = self.cpu.ie.set_high(value),
            0x04000208 => self.cpu.ime = value & 1 != 0,

            _ => (),
        }
    }

    /// Write a half-word from the bus (LE). Does no timing-related things; simply sets the value.
    /// Also does not handle unaligned writes (yet)
    fn write_hword(&mut self, addr: u32, value: u16) {
        self.write_byte(addr, value.low());
        self.write_byte(addr + 1, value.high());
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply sets the value.
    /// Also does not handle unaligned writes (yet)
    fn write_word(&mut self, addr: u32, value: u32) {
        self.write_hword(addr, value.low());
        self.write_hword(addr + 2, value.high());
    }
}

impl Default for GameGirlAdv {
    fn default() -> Self {
        Self {
            cpu: Cpu {
                mode: CpuMode::Arm,
                regs: Regs {
                    low: [0; 8],
                    high: [FiqReg::default(); 5],
                    sp: ModeReg::default(),
                    lr: ModeReg::default(),
                    pc: 0,
                    cpsr: 0,
                    spsr: ModeReg::default(),
                },
                ie: 0,
                if_: 0,
                ime: false,
            },
            memory: Memory {
                bios: [0; 16 * KB],
                wram1: [0; 256 * KB],
                wram2: [0; 32 * KB],
            },
            gpu: Ppu {
                regs: [0; 56],
                palette: [0; KB],
                vram: [0; 96 * KB],
                oam: [0; KB],
            },
            apu: Apu {
                regs: [0; APU_REG_END - APU_REG_START],
            },
            // Meh...
            dma: [
                Dma { regs: [0; 10] },
                Dma { regs: [0; 10] },
                Dma { regs: [0; 10] },
                Dma { regs: [0; 10] },
            ],
            timers: [
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
            ],
            input: Input { regs: [0; 4] },
            cart: Cartridge {},

            options: EmulateOptions::default(),
            config: GGConfig::default(),
        }
    }
}
