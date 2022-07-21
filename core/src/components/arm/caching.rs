use std::sync::Arc;

use crate::{
    components::arm::{inst_arm::ArmHandler, inst_thumb::ThumbHandler, interface::ArmSystem},
    numutil::NumExt,
};

const IWRAM_PAGE_SIZE: u32 = 128;
const IWRAM_END: u32 = 0x300_7FFF - 0x400;

pub struct Cache<S: ArmSystem> {
    bios: Vec<Option<CacheEntry<S>>>,
    rom: Vec<Option<CacheEntry<S>>>,

    iwram: Vec<Option<CacheEntry<S>>>,
    iwram_cache_indices: Vec<Vec<u32>>,

    pub enabled: bool,
}

impl<S: ArmSystem> Cache<S> {
    pub fn get(&self, pc: u32) -> Option<CacheEntry<S>> {
        match pc {
            0..=0x3FFF => self.bios.get(pc.us() >> 1).cloned().flatten(),
            0x300_0000..=IWRAM_END => self.iwram.get((pc.us() & 0x7FFF) >> 1).cloned().flatten(),
            0x800_0000..=0xDFF_FFFF => self.rom.get((pc.us() - 0x800_0000) >> 1).cloned().flatten(),
            _ => None,
        }
    }

    pub fn put(&mut self, pc: u32, entry: CacheEntry<S>) {
        match pc {
            0..=0x3FFF => Self::insert(&mut self.bios, pc >> 1, entry),
            0x300_0000..=IWRAM_END => {
                let location = (pc & 0x7FFF) >> 1;
                Self::insert(&mut self.iwram, location, entry);
                self.iwram_cache_indices[location.us() / IWRAM_PAGE_SIZE.us()].push(location);
            }
            0x800_0000..=0xDFF_FFFF => Self::insert(&mut self.rom, (pc - 0x800_0000) >> 1, entry),
            _ => (),
        }
    }

    fn insert(set: &mut [Option<CacheEntry<S>>], location: u32, entry: CacheEntry<S>) {
        set[location.us()] = Some(entry);
    }

    pub fn init(&mut self, cart_size: usize) {
        self.bios.resize(0x2000, None);
        self.iwram.resize(0x4000, None);
        self.iwram_cache_indices
            .resize(0x4000 / IWRAM_PAGE_SIZE.us(), Vec::new());
        self.rom.resize(cart_size >> 1, None);
        self.enabled = true;
    }

    pub fn force_end_block(pc: u32) -> bool {
        (0x300_0000..=0x3FF_FFFF).contains(&pc) && (pc & (IWRAM_PAGE_SIZE - 1)) == 0
    }

    pub fn write(&mut self, addr: u32) {
        if !self.iwram.is_empty() && (0x300_0000..=IWRAM_END).contains(&addr) {
            let location = (addr & 0x7FFF) >> 1;
            for entry in self.iwram_cache_indices[location.us() / IWRAM_PAGE_SIZE.us()].drain(..) {
                self.iwram[entry.us()] = None;
            }
        }
    }

    pub fn invalidate_rom(&mut self) {
        if !self.enabled {
            return;
        }
        let len = self.rom.len();
        unsafe { self.rom.set_len(0) };
        self.rom.resize(len, None);
        log::trace!("ROM cache invalidated: WAITCNT changed.");
    }
}

impl<S: ArmSystem> Default for Cache<S> {
    fn default() -> Self {
        Self {
            bios: Vec::default(),
            rom: Vec::default(),
            iwram: Vec::default(),
            iwram_cache_indices: Vec::default(),
            enabled: false,
        }
    }
}

pub enum CacheEntry<S: ArmSystem> {
    Arm(Arc<Vec<CachedInst<u32, ArmHandler<S>>>>),
    Thumb(Arc<Vec<CachedInst<u16, ThumbHandler<S>>>>),
}

impl<S: ArmSystem> Clone for CacheEntry<S> {
    fn clone(&self) -> Self {
        match self {
            Self::Arm(arg0) => Self::Arm(arg0.clone()),
            Self::Thumb(arg0) => Self::Thumb(arg0.clone()),
        }
    }
}

pub struct CachedInst<I, H> {
    pub inst: I,
    pub handler: H,
    pub sn_cycles: u16,
}
