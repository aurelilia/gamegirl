use alloc::vec::Vec;
use core::mem;

use common::Time;

use super::CacheIndex;
use crate::{arm::ArmHandler, interface::Bus, thumb::ThumbHandler, Cpu};

pub enum CacheStatus {
    JustInterpret,
    MakeCacheNow,
    RunCacheNowAt(CacheIndex),
}

pub struct CacheEntry<S: Bus> {
    pub(super) entry: CacheEntryKind<S>,
}

impl<S: Bus> CacheEntry<S> {
    pub fn borrow(&self) -> &'static CacheEntryKind<S> {
        unsafe { mem::transmute(&self.entry) }
    }
}

pub enum CacheEntryKind<S: Bus> {
    Arm(Vec<CachedInstruction<u32, ArmHandler<Cpu<S>>>>),
    Thumb(Vec<CachedInstruction<u16, ThumbHandler<Cpu<S>>>>),
    FailedRetry,
}

pub struct CachedInstruction<I, H> {
    pub instruction: I,
    pub handler: H,
    pub cycles: Time,
}
