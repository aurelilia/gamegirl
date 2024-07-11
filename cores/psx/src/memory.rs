// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{
    ops::{Index, IndexMut},
    ptr,
};

use common::{
    components::memory::{MemoryMappedSystem, MemoryMapper},
    numutil::{hword, word, NumExt, U16Ext, U32Ext},
};

use crate::{
    addr::{DMABASE, DMACTRL, DMAINT, GP0, GP1, GPUREAD, GPUSTAT, MMIOBASE},
    dma::Dma,
    gpu::Gpu,
    PlayStation,
};

const KB: usize = 1024;
const MB: usize = KB * KB;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    ram: [u8; 2 * MB],
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    scratchpad: [u8; KB],
    pub bios: Box<[u8]>,
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    pub mmio: [u32; 8 * KB / 4],
}

impl PlayStation {
    pub fn get<T: NumExt>(&mut self, addr: u32) -> T {
        let phys = Self::phys_addr(addr);
        match phys {
            0x0000_0000..=0x007F_FFFF => Self::raw_read(&self.memory.ram, phys & 0x1F_FFFF),
            0x1F80_0000..=0x1F80_03FF => {
                Self::raw_read(&self.memory.scratchpad, phys - 0x1F80_0000)
            }
            0x1F80_1000..=0x1F80_1FFF => match phys - MMIOBASE {
                GPUREAD => T::from_u32(self.ppu.read),
                // TODO fix
                GPUSTAT => T::from_u32(Into::<u32>::into(self.ppu.stat).set_bit(19, false)),

                _ => {
                    let ptr = self.memory.mmio.as_ptr() as *const u8;
                    unsafe {
                        let ptr = ptr.add(phys.us() - 0x1F80_1000);
                        ptr::read(ptr as *const T)
                    }
                }
            },
            0x1FC0_0000..=0x1FC7_FFFF => Self::raw_read(&self.memory.bios, phys - 0x1FC0_0000),

            unknown => {
                log::warn!(
                    "Read from unmapped address 0x{addr:X} (physical address 0x{unknown:X}), reading max value"
                );
                T::from_u32(0xFFFF_FFFF)
            }
        }
    }

    pub fn set<T: NumExt>(&mut self, addr: u32, value: T) {
        let phys = Self::phys_addr(addr);
        self.debugger.write_occurred(addr);
        match phys {
            0x0000_0000..=0x007F_FFFF => {
                Self::raw_write(&mut self.memory.ram, phys & 0x1F_FFFF, value)
            }
            0x1F80_0000..=0x1F80_03FF => {
                Self::raw_write(&mut self.memory.scratchpad, phys - 0x1F80_0000, value)
            }
            0x1F80_1000..=0x1F80_1FFF => self.set_io(phys - MMIOBASE, value),
            0x1FC0_0000..=0x1FC7_FFFF => {
                Self::raw_write(&mut self.memory.bios, phys - 0x1FC0_0000, value)
            }

            unknown => {
                log::warn!(
                    "Write to unmapped address 0x{addr:X} (physical address 0x{unknown:X}), ignoring"
                );
            }
        }
    }

    fn raw_read<T: NumExt>(arr: &[u8], offset: u32) -> T {
        unsafe { ptr::read(Self::raw_ptr(arr, offset)) }
    }

    fn raw_write<T: NumExt>(arr: &mut [u8], offset: u32, value: T) {
        unsafe { ptr::write(Self::raw_ptr_mut(arr, offset), value) }
    }

    fn raw_ptr<T: NumExt>(arr: &[u8], offset: u32) -> *const T {
        assert!((offset + T::WIDTH).us() <= arr.len());
        &arr[offset.us()] as *const u8 as *const T
    }

    fn raw_ptr_mut<T: NumExt>(arr: &mut [u8], offset: u32) -> *mut T {
        assert!((offset + T::WIDTH).us() <= arr.len());
        &mut arr[offset.us()] as *mut u8 as *mut T
    }

    pub fn set_io<T: NumExt>(&mut self, addr: u32, value: T) {
        let value = value.u32(); // TODO not all MMIO is 32b
        match addr {
            // DMA
            DMAINT => self[DMAINT] = value & 0xFFFF_803F,
            // Address register. Upper bits unused
            _ if (addr > DMABASE && addr < DMACTRL) && addr & 0xF == 0 => {
                self[addr] = value & 0xFF_FFFF
            }
            // Channel control register.
            _ if (addr > DMABASE && addr < DMACTRL) && addr & 0xF == 8 => {
                self[addr] = value; // TODO some bits are supposed to be always
                                    // 0...
                Dma::maybe_trigger(self, addr);
            }

            // GPU
            GP0 => Gpu::gp0_write(self, value.u32()),
            GP1 => Gpu::gp1_write(self, value.u32()),

            _ => self[addr] = value,
        }
    }

    fn phys_addr(addr: u32) -> u32 {
        const MASKS: [u32; 8] = [
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
            0x7FFF_FFFF,
            0x1FFF_FFFF,
            0xFFFF_FFFF,
            0xFFFF_FFFF,
        ];
        let mask = MASKS[addr.bits(29, 3).us()];
        addr & mask
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ram: [0; 2 * MB],
            scratchpad: [0; KB],
            bios: Box::new([]),
            mmio: [0; 2 * KB],
        }
    }
}

impl Index<u32> for PlayStation {
    type Output = u32;

    fn index(&self, addr: u32) -> &Self::Output {
        assert!(addr < 0x1FFF);
        &self.memory.mmio[(addr / 4).us()]
    }
}

impl IndexMut<u32> for PlayStation {
    fn index_mut(&mut self, addr: u32) -> &mut Self::Output {
        assert!(addr < 0x1FFF);
        &mut self.memory.mmio[(addr / 4).us()]
    }
}
