use crate::gga::audio::{APUREG_END, APUREG_START};
use crate::gga::cpu::{CPUMode, Regs};
use crate::gga::dma::{DMA, DMA_END, DMA_START, DMA_WIDTH};
use crate::gga::graphics::{
    GPUREG_END, GPUREG_START, OAM_END, OAM_START, PALETTE_END, PALETTE_START, VRAM_END, VRAM_START,
};
use crate::gga::input::{Input, INPUT_END, INPUT_START};
use crate::gga::memory::{
    BIOS_END, BIOS_START, KB, WRAM1_END, WRAM1_START, WRAM2_END, WRAM2_START,
};
use crate::gga::timer::{Timer, TIMER_END, TIMER_START, TIMER_WIDTH};
use crate::numutil::{hword, word, U16Ext, U32Ext};
use audio::APU;
use cartridge::Cartridge;
use cpu::CPU;
use graphics::GPU;
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
#[derive(Debug, Clone)]
pub struct GameGirlAdv {
    pub cpu: CPU,
    pub memory: Memory,
    pub gpu: GPU,
    pub apu: APU,
    pub dma: [DMA; 4],
    pub timers: [Timer; 4],
    pub input: Input,
    pub cart: Cartridge,
}

impl GameGirlAdv {
    /// Step forward the emulated console including all subsystems.
    pub fn step(&mut self) {
        todo!()
    }

    /// Read a byte from the bus. Does no timing-related things; simply fetches the value.
    fn read_byte(&self, addr: usize) -> u8 {
        match addr {
            BIOS_START..=BIOS_END => self.memory.bios[addr - BIOS_START],
            WRAM1_START..=WRAM1_END => self.memory.wram1[addr - WRAM1_START],
            WRAM2_START..=WRAM2_END => self.memory.wram2[addr - WRAM2_START],

            GPUREG_START..=GPUREG_END => self.gpu.regs[addr - GPUREG_START],
            PALETTE_START..=PALETTE_END => self.gpu.palette[addr - PALETTE_START],
            VRAM_START..=VRAM_END => self.gpu.vram[addr - VRAM_START],
            OAM_START..=OAM_END => self.gpu.oam[addr - OAM_START],

            APUREG_START..=APUREG_END => self.apu.regs[addr - APUREG_START],
            DMA_START..=DMA_END => {
                let dma_idx = (addr - DMA_START) / DMA_WIDTH;
                self.dma[dma_idx].regs[addr - DMA_START - (dma_idx * DMA_WIDTH)]
            }
            TIMER_START..=TIMER_END => {
                let timer_idx = (addr - TIMER_START) / TIMER_WIDTH;
                self.timers[timer_idx].regs[addr - TIMER_START - (timer_idx * TIMER_WIDTH)]
            }
            INPUT_START..=INPUT_END => self.input.regs[addr - INPUT_START],

            0x04000200 => self.cpu.IE.low(),
            0x04000201 => self.cpu.IE.high(),
            0x04000202 => self.cpu.IF.low(),
            0x04000203 => self.cpu.IF.high(),
            0x04000208 => self.cpu.IME as u8,
            0x04000209 => 0, // High unused bits of IME

            _ => 0xFF,
        }
    }

    /// Read a half-word from the bus (LE). Does no timing-related things; simply fetches the value.
    /// Also does not handle unaligned reads (yet)
    fn read_hword(&self, addr: usize) -> u16 {
        hword(self.read_byte(addr), self.read_byte(addr + 1))
    }

    /// Read a word from the bus (LE). Does no timing-related things; simply fetches the value.
    /// Also does not handle unaligned reads (yet)
    fn read_word(&self, addr: usize) -> u32 {
        word(self.read_hword(addr), self.read_hword(addr + 2))
    }

    /// Write a byte to the bus. Does no timing-related things; simply sets the value.
    fn write_byte(&mut self, addr: usize, value: u8) {
        match addr {
            BIOS_START..=BIOS_END => self.memory.bios[addr - BIOS_START] = value,
            WRAM1_START..=WRAM1_END => self.memory.wram1[addr - WRAM1_START] = value,
            WRAM2_START..=WRAM2_END => self.memory.wram2[addr - WRAM2_START] = value,

            GPUREG_START..=GPUREG_END => self.gpu.regs[addr - GPUREG_START] = value,
            PALETTE_START..=PALETTE_END => self.gpu.palette[addr - PALETTE_START] = value,
            VRAM_START..=VRAM_END => self.gpu.vram[addr - VRAM_START] = value,
            OAM_START..=OAM_END => self.gpu.oam[addr - OAM_START] = value,

            APUREG_START..=APUREG_END => self.apu.regs[addr - APUREG_START] = value,
            DMA_START..=DMA_END => {
                let dma_idx = (addr - DMA_START) / DMA_WIDTH;
                self.dma[dma_idx].regs[addr - DMA_START - (dma_idx * DMA_WIDTH)] = value;
            }
            TIMER_START..=TIMER_END => {
                let timer_idx = (addr - TIMER_START) / TIMER_WIDTH;
                self.timers[timer_idx].regs[addr - TIMER_START - (timer_idx * TIMER_WIDTH)] = value;
            }
            INPUT_START..=INPUT_END => self.input.regs[addr - INPUT_START] = value,

            0x04000201 => self.cpu.IE = self.cpu.IE.set_low(value),
            0x04000202 => self.cpu.IF = self.cpu.IF.set_high(value),
            0x04000203 => self.cpu.IF = self.cpu.IF.set_low(value),
            0x04000200 => self.cpu.IE = self.cpu.IE.set_high(value),
            0x04000208 => self.cpu.IME = value & 1 != 0,

            _ => (),
        }
    }

    /// Write a half-word from the bus (LE). Does no timing-related things; simply sets the value.
    /// Also does not handle unaligned writes (yet)
    fn write_hword(&mut self, addr: usize, value: u16) {
        self.write_byte(addr, value.low());
        self.write_byte(addr + 1, value.high());
    }

    /// Write a word from the bus (LE). Does no timing-related things; simply sets the value.
    /// Also does not handle unaligned writes (yet)
    fn write_word(&mut self, addr: usize, value: u32) {
        self.write_hword(addr, value.low());
        self.write_hword(addr + 2, value.high());
    }

    /// Create a new console.
    pub fn new() -> Self {
        Self {
            cpu: CPU {
                mode: CPUMode::Arm,
                regs: Regs {
                    low: [0; 8],
                    high: [Default::default(); 5],
                    sp: Default::default(),
                    lr: Default::default(),
                    pc: 0,
                    cpsr: 0,
                    spsr: Default::default(),
                },
                IE: 0,
                IF: 0,
                IME: false,
            },
            memory: Memory {
                bios: [0; 16 * KB],
                wram1: [0; 256 * KB],
                wram2: [0; 32 * KB],
            },
            gpu: GPU {
                regs: [0; 56],
                palette: [0; KB],
                vram: [0; 96 * KB],
                oam: [0; KB],
            },
            apu: APU {
                regs: [0; APUREG_END - APUREG_START],
            },
            // Meh...
            dma: [
                DMA { regs: [0; 10] },
                DMA { regs: [0; 10] },
                DMA { regs: [0; 10] },
                DMA { regs: [0; 10] },
            ],
            timers: [
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
                Timer { regs: [0; 4] },
            ],
            input: Input { regs: [0; 4] },
            cart: Cartridge {},
        }
    }
}
