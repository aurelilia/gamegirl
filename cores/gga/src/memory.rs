// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, self file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with self file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use alloc::boxed::Box;
use core::mem;

use armchair::{
    access::{CODE, NONSEQ, SEQ},
    interface::{Bus, RwType},
    state::Flag,
    Access, Address,
};
use common::{
    components::thin_pager::{ThinPager, RO, RW},
    numutil::{hword, word, ByteArrayExt, NumExt, U16Ext},
};
use modular_bitfield::{bitfield, specifiers::*};

use crate::{
    cpu::GgaFullBus,
    hw::{bios::BIOS, input::KeyControl},
    GgaBus,
};

pub const KB: usize = 1024;

pub const MAX_ADDRESS: Address = Address(0xFFF_FFFF);

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WaitCnt {
    sram: B2,
    ws0_n: B2,
    ws0_s: B1,
    ws1_n: B2,
    ws1_s: B1,
    ws2_n: B2,
    ws2_s: B1,
    #[skip]
    phi: B2,
    #[skip]
    __: B1,
    prefetch_en: bool,
    #[skip]
    __: B1,
}

#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Prefetch {
    pub active: bool,
    pub restart: bool,
    thumb: bool,

    head: Address,
    tail: Address,

    count: u32,
    countdown: i16,
    duty: u16,
}

/// Memory struct containing the GGA's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    pub bios: Box<[u8]>,
    pub ewram: Box<[u8]>,
    pub iwram: Box<[u8]>,

    // Various registers
    pub keycnt: KeyControl,
    pub keys_prev: u16,
    pub waitcnt: WaitCnt,
    /// Value to return when trying to read BIOS outside of it
    pub(crate) bios_value: u32,

    pub(crate) prefetch: Prefetch,
    pager: ThinPager,
    wait_word: [u16; 32],
    wait_other: [u16; 32],
}

impl GgaFullBus<'_> {
    pub fn get<T: RwType>(&self, addr_unaligned: Address) -> T {
        let addr = addr_unaligned.align(T::WIDTH);
        if addr > MAX_ADDRESS {
            return T::from_u32(self.invalid_read::<false>(addr));
        }
        if let Some(read) = self.memory.pager.read(addr.0) {
            return read;
        }

        let region = addr.0 >> 24;
        let a = addr.0.us();
        match region {
            // Basic
            0x00 => self.bios_read(addr),
            0x05 => self.ppu.palette.get_wrap(a),
            0x07 => self.ppu.oam.get_wrap(a),

            // MMIO
            0x04 => self.get_mmio(addr),
            // VRAM with weird mirroring, TODO in mapper
            0x06 => {
                let a = a & 0x1_FFFF;
                if a < 0x1_8000 {
                    self.ppu.vram.get_exact(a)
                } else {
                    self.ppu.vram[0x1_0000..].get_exact(a - 0x1_8000)
                }
            }

            // Cart save
            // EEPROM
            0x0D if T::WIDTH == 2 && self.cart.is_eeprom_at(addr) => {
                T::from_u16(self.cart.read_ram_hword())
            }
            // Flash / SRAM
            0x0E | 0x0F => {
                // Reading [half]words causes the byte to be repeated
                let byte = self.cart.read_ram_byte(addr_unaligned.0.us() & 0xFFFF);
                match T::WIDTH {
                    1 => T::from_u8(byte),
                    2 => T::from_u16(hword(byte, byte)),
                    4 => T::from_u32(word(hword(byte, byte), hword(byte, byte))),
                    _ => unreachable!(),
                }
            }

            // Cart
            0x08..=0x0D if let Some(v) = self.cart.rom.try_get_exact(a & 0x1FF_FFFF) => v,
            // 1MB carts are special and wrap
            0x08..0x0D if self.cart.rom.len() == (2 << 19) => {
                self.cart.rom.get_wrap(a & 0x1FF_FFFF)
            }

            _ if T::WIDTH == 4 => T::from_u32(self.invalid_read::<true>(addr)),
            _ => T::from_u32(self.invalid_read::<false>(addr)),
        }
    }

    pub(super) fn invalid_read<const WORD: bool>(&self, addr: Address) -> u32 {
        let shift = (addr.0 & 3) << 3;
        let value = match addr.0 {
            0x0800_0000..=0x0DFF_FFFF => {
                // Out of bounds ROM read
                let addr = (addr.0 & !if WORD { 3 } else { 1 }) >> 1;
                let low = addr.u16();
                return word(low, low.wrapping_add(1));
            }

            _ if self.cpu.pc() == self.dma.pc_at_last_end => self.dma.cache,

            _ => {
                // Open bus
                let pc_addr = self.cpu.pc();
                let pc = pc_addr.0;
                if pc > 0xFFF_FFFF
                    || (0x3FFF..0x200_0000).contains(&pc)
                    || (0x400_0000..0x500_0000).contains(&pc)
                {
                    return 0;
                }

                if !self.cpu.is_flag(Flag::Thumb) {
                    // Simple case: just read PC in ARM mode
                    self.get(self.cpu.pc())
                } else {
                    // Thumb mode... complicated.
                    // https://problemkaputt.de/gbatek.htm#gbaunpredictablethings
                    match pc >> 24 {
                        0x02 | 0x05 | 0x06 | 0x08..=0xD => {
                            let hword = self.get(self.cpu.pc());
                            word(hword, hword)
                        }
                        _ if pc.is_bit(1) => {
                            word(self.get(pc_addr - Address::HW), self.get(pc_addr))
                        }
                        0x00 | 0x07 => word(self.get(pc_addr), self.get(pc_addr + Address::HW)),
                        0x03 => word(self.get(pc_addr), self.get(pc_addr - Address::HW)),

                        _ => unreachable!(),
                    }
                }
            }
        };
        value >> shift
    }

    pub fn set<T: RwType>(&mut self, addr_unaligned: Address, value: T) {
        let addr = addr_unaligned.align(T::WIDTH);
        if addr > MAX_ADDRESS {
            return;
        }

        if let Some(write) = self.memory.pager.write(addr.0) {
            // TODO
            // self.cpu.cache.invalidate_address(addr);
            *write = value;
            return;
        }

        let region = addr.0 >> 24;
        let a = addr.0.us();
        match region {
            // MMIO
            0x04 => self.set_mmio(addr, value),
            // VRAM with weird mirroring and byte write behavior
            0x05..=0x07 if T::WIDTH == 1 => {
                let value = value.u8();
                match addr.0 {
                    0x0500_0000..=0x0600_FFFF if !self.ppu.regs.is_bitmap_mode() => {
                        self.set(addr.align(2), hword(value, value))
                    }
                    0x0500_0000..=0x0601_3FFF => self.set(addr.align(2), hword(value, value)),
                    0x0602_0000..=0x06FF_FFFF if addr.0 & 0x1_FFFF < 0x1_0000 => {
                        // Only BG VRAM gets written to, OBJ VRAM is ignored
                        self.set(addr.align(2), hword(value, value));
                    }
                    _ => (), // Ignored
                };
            }
            0x05 => self.ppu.palette.set_wrap(a, value),
            0x06 => {
                let a = a & 0x1_FFFF;
                if a < 0x1_8000 {
                    self.ppu.vram.set_exact(a, value)
                } else {
                    self.ppu.vram[0x1_0000..].set_exact(a - 0x1_8000, value)
                }
            }
            0x07 => self.ppu.oam.set_wrap(a, value),

            // Cart save
            // EEPROM
            0x0D if T::WIDTH == 2 && self.cart.is_eeprom_at(addr) => {
                self.cart.write_ram_hword(value.u16());
            }
            // Flash / SRAM
            0x0E | 0x0F => {
                let byte = match T::WIDTH {
                    1 => value.u8(),
                    2 if addr_unaligned.0.is_bit(0) => value.u16().high(),
                    2 => value.u8(),
                    4 => {
                        let byte_shift = (addr_unaligned.0 & 3) * 8;
                        (value.u32() >> byte_shift).u8()
                    }
                    _ => unreachable!(),
                };
                self.cart
                    .write_ram_byte(addr_unaligned.0.us() & 0xFFFF, byte);
            }

            _ => (),
        }
    }

    fn bios_read<T: NumExt>(&self, addr: Address) -> T {
        if addr >= Address(0x4000) {
            return T::from_u32(self.invalid_read::<false>(addr));
        }

        if self.cpu.pc() < Address(0x100_0000) {
            self.memory.bios.get_wrap(addr.0.us())
        } else {
            T::from_u32(self.memory.bios_value)
        }
    }

    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        self.update_wait_times();

        let bus = &mut self.bus;
        let pager = &mut bus.memory.pager;
        pager.init(0xFFF_FFFF);
        pager.map(&bus.memory.ewram, 0x200_0000..0x300_0000, RW);
        pager.map(&bus.memory.iwram, 0x300_0000..0x400_0000, RW);
        // PAL, OAM, Writes and VRAM mirroring are in the slow path
        pager.map(&bus.ppu.vram, 0x600_0000..0x601_8000, RO);
        // Cap at end due to EEPROM
        let rom_len = bus.cart.rom.len().u32();
        pager.map(&bus.cart.rom, 0x800_0000..(0x800_0000 + rom_len), RO);
        pager.map(&bus.cart.rom, 0xA00_0000..(0xA00_0000 + rom_len), RO);
        pager.map(
            &bus.cart.rom,
            0xC00_0000..(0xC00_0000 + rom_len).min(0x0DFF_C000),
            RO,
        );

        if self.c.config.cached_interpreter {
            // TODO
            // self.cpu.cache.init();
        }
    }

    /// Get wait time for a given address.
    #[inline]
    pub fn wait_time<T: NumExt + 'static>(&mut self, addr: Address, ty: Access) -> u16 {
        let region = addr.0.us() >> 24;
        let wait = self.wait_time_inner::<T>(addr, ty);
        match region {
            0x08..=0x0D => self.handle_prefetch::<T>(addr, ty, wait),
            0x0E..=0x0F => {
                self.stop_prefetch();
                wait
            }
            0x10.. => 1,
            _ => wait,
        }
    }

    fn handle_prefetch<T: NumExt + 'static>(
        &mut self,
        addr: Address,
        ty: Access,
        mut regular: u16,
    ) -> u16 {
        if (ty & CODE) == 0 {
            self.stop_prefetch();
            return regular;
        }

        let pf = &mut self.memory.prefetch;
        if pf.active {
            // Value is head of buffer
            if pf.count != 0 && addr == pf.head {
                pf.count -= 1;
                pf.head += Address(T::WIDTH);
                return 1;
            }
            // Value is being prefetched
            if pf.countdown > 0 && addr == pf.tail {
                pf.head = pf.tail;
                pf.count = 0;
                return pf.countdown as u16;
            }
        }

        self.stop_prefetch();

        // Prefetch should keep transfer alive
        if self.memory.waitcnt.prefetch_en() {
            let duty = if self.cpu.is_flag(Flag::Thumb) {
                self.wait_time_inner::<u16>(addr, SEQ | CODE)
            } else {
                self.wait_time_inner::<u32>(addr, SEQ | CODE)
            };

            let pf = &mut self.memory.prefetch;
            if pf.restart {
                pf.restart = false;
                // Force non-seq
                regular = self.wait_time_inner::<T>(addr, CODE);
            }

            let pf = &mut self.bus.memory.prefetch;
            pf.thumb = self.cpu.is_flag(Flag::Thumb);
            pf.tail = addr + Address(T::WIDTH);
            pf.head = pf.tail;
            pf.active = true;
            pf.count = 0;
            pf.duty = duty;
            pf.countdown = duty as i16;
        }

        regular
    }

    pub(super) fn stop_prefetch(&mut self) {
        let prefetch = &mut self.bus.memory.prefetch;
        if prefetch.active {
            // Penalty for accessing ROM/RAM during last cycle of prefetch fetch
            if (0x800_0000..0xE00_0000).contains(&self.cpu.pc().0) {
                let duty = prefetch.duty / 2 + 1;
                if prefetch.countdown == 1 || (!prefetch.thumb && duty == prefetch.countdown as u16)
                {
                    self.tick(1);
                    self.cpu.access_type = NONSEQ;
                }
            }
            self.memory.prefetch.active = false;
        }
    }

    fn wait_time_inner<T: NumExt + 'static>(&mut self, addr: Address, ty: Access) -> u16 {
        let region = (addr.0.us() >> 24) & 0xF;
        let ty_idx = if ty & SEQ != 0 { 16 } else { 0 };
        if T::WIDTH == 4 {
            self.memory.wait_word[region + ty_idx]
        } else {
            self.memory.wait_other[region + ty_idx]
        }
    }

    pub(super) fn update_wait_times(&mut self) {
        for i in 0..16 {
            let addr = i.u32() * 0x100_0000;
            self.memory.wait_word[i] = self.calc_wait_time::<4>(addr, NONSEQ);
            self.memory.wait_other[i] = self.calc_wait_time::<2>(addr, NONSEQ);
            self.memory.wait_word[i + 16] = self.calc_wait_time::<4>(addr, SEQ);
            self.memory.wait_other[i + 16] = self.calc_wait_time::<2>(addr, SEQ);
        }
    }

    const WS_NONSEQ: [u16; 4] = [5, 4, 3, 9];

    fn calc_wait_time<const W: u32>(&self, addr: u32, ty: Access) -> u16 {
        match (addr, W, ty) {
            (0x0200_0000..=0x02FF_FFFF, 4, _) => 6,
            (0x0200_0000..=0x02FF_FFFF, _, _) => 3,
            (0x0500_0000..=0x06FF_FFFF, 4, _) => 2,

            (0x0800_0000..=0x0DFF_FFFF, 4, _) => {
                // Cart bus is 16bit, word access is therefore 2x
                self.calc_wait_time::<2>(addr, ty) + self.calc_wait_time::<2>(addr, SEQ)
            }

            (0x0800_0000..=0x09FF_FFFF, _, SEQ) => 3 - self.memory.waitcnt.ws0_s().u16(),
            (0x0800_0000..=0x09FF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws0_n().us()]
            }

            (0x0A00_0000..=0x0BFF_FFFF, _, SEQ) => 5 - (self.memory.waitcnt.ws1_s().u16() * 3),
            (0x0A00_0000..=0x0BFF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws1_n().us()]
            }

            (0x0C00_0000..=0x0DFF_FFFF, _, SEQ) => 9 - (self.memory.waitcnt.ws2_s().u16() * 7),
            (0x0C00_0000..=0x0DFF_FFFF, _, NONSEQ) => {
                Self::WS_NONSEQ[self.memory.waitcnt.ws2_n().us()]
            }

            (0x0E00_0000..=0x0EFF_FFFF, _, _) => Self::WS_NONSEQ[self.memory.waitcnt.sram().us()],

            _ => 1,
        }
    }
}

impl GgaBus {
    pub(super) fn step_prefetch(&mut self, count: u16) {
        let pf = &mut self.memory.prefetch;
        if pf.active {
            pf.countdown -= count as i16;
            while pf.countdown <= 0 {
                let capacity = if pf.thumb { 8 } else { 4 };
                let size = if pf.thumb { Address::HW } else { Address::WORD };
                pf.countdown += pf.duty as i16;
                if self.memory.waitcnt.prefetch_en() && pf.count < capacity {
                    pf.count += 1;
                    pf.tail += size;
                }
            }
        }
    }
}

impl Memory {
    pub fn get_fastmem<T: Copy>(&self, addr_unaligned: u32) -> Option<T> {
        let addr = addr_unaligned & !(mem::size_of::<T>().u32() - 1);
        self.pager.read(addr)
    }

    pub fn get_fastmem_raw(&mut self, addr: u32) -> Option<*mut u8> {
        let ptr = self.pager.get_raw(addr).ptr;
        (!ptr.is_null()).then_some(ptr)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            bios: BIOS.into(),
            ewram: Box::new([0; 256 * KB]),
            iwram: Box::new([0; 32 * KB]),
            keycnt: 0.into(),
            keys_prev: 0,
            waitcnt: 0.into(),
            bios_value: 0xE129_F000,
            prefetch: Prefetch::default(),
            pager: ThinPager::default(),
            wait_word: [0; 32],
            wait_other: [0; 32],
        }
    }
}

unsafe impl Send for Memory {}
