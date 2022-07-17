use std::sync::Arc;

use crate::{
    gga::cpu::{inst_arm::ArmHandler, inst_thumb::ThumbHandler},
    numutil::NumExt,
};

#[derive(Default)]
pub struct Cache {
    bios: Vec<Option<CacheEntry>>,
    rom: Vec<Option<CacheEntry>>,
}

impl Cache {
    pub fn get(&self, pc: u32) -> Option<CacheEntry> {
        match pc {
            0..=0x3FFF => self.bios.get(pc.us() >> 1).cloned().flatten(),
            0x800_0000..=0xDFF_FFFF => self.rom.get((pc.us() - 0x800_0000) >> 1).cloned().flatten(),
            _ => None,
        }
    }

    pub fn put(&mut self, pc: u32, entry: CacheEntry) {
        match pc {
            0..=0x3FFF => Self::insert(&mut self.bios, pc >> 1, entry),
            0x800_0000..=0xDFF_FFFF => Self::insert(&mut self.rom, (pc - 0x800_0000) >> 1, entry),
            _ => panic!("Not cacheable PC!"),
        }
    }

    fn insert(set: &mut [Option<CacheEntry>], location: u32, entry: CacheEntry) {
        set[location.us()] = Some(entry);
    }

    pub fn can_make_cache(pc: u32) -> bool {
        !(0x100_0000..=0x800_0000).contains(&pc)
    }

    pub fn init(&mut self, cart_size: usize) {
        self.bios.resize(0x2000, None);
        self.rom.resize(cart_size >> 1, None);
    }
}

#[derive(Clone)]
pub enum CacheEntry {
    Arm(Arc<Vec<CachedInst<u32, ArmHandler>>>),
    Thumb(Arc<Vec<CachedInst<u16, ThumbHandler>>>),
}

pub struct CachedInst<I, H> {
    pub inst: I,
    pub handler: H,
    pub sn_cycles: u16,
}
