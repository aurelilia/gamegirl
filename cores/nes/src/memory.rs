// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ptr;

use common::{
    components::memory::{MemoryMappedSystem, MemoryMapper},
    numutil::{hword, NumExt},
};

use crate::{cpu::Reg::*, Nes};

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    mapper: MemoryMapper<256>,
}

impl Nes {
    pub fn read_imm(&mut self) -> u8 {
        let value = self.read(self.cpu.pc);
        self.cpu.pc += 1;
        value
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        self.advance_clock(1);
        self.get(addr)
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        self.advance_clock(1);
        self.set(addr, value);
    }

    pub fn push(&mut self, value: u8) {
        let stack = self.cpu.get(S);
        self.write(hword(0x01, stack), value);
        self.cpu.set(S, stack.wrapping_sub(1));
        self.advance_clock(1);
    }

    pub fn pop(&mut self) -> u8 {
        self.advance_clock(2);
        let stack = self.cpu.get(S).wrapping_add(1);
        self.cpu.set(S, stack);
        self.read(hword(0x01, stack))
    }

    pub fn get<T: NumExt>(&mut self, addr: u16) -> T {
        T::from_u16(addr)
    }

    pub fn set(&mut self, addr: u16, value: u8) {}
}

impl MemoryMappedSystem<256> for Nes {
    type Usize = u16;
    const ADDR_MASK: &'static [usize] = &[0xFF];
    const PAGE_POW: usize = 8;
    const MASK_POW: usize = 0;

    fn get_mapper(&self) -> &MemoryMapper<256> {
        &self.mem.mapper
    }

    fn get_mapper_mut(&mut self) -> &mut MemoryMapper<256> {
        &mut self.mem.mapper
    }

    unsafe fn get_page<const R: bool>(&self, a: usize) -> *mut u8 {
        unsafe fn offs(reg: &[u8], offs: usize) -> *mut u8 {
            let ptr = reg.as_ptr() as *mut u8;
            ptr.add(offs)
        }

        if !R {
            return ptr::null::<u8>() as *mut u8;
        }

        match a {
            _ => ptr::null::<u8>() as *mut u8,
        }
    }
}
