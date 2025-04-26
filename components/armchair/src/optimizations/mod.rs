use cache::Cache;
use waitloop::WaitloopData;

use crate::interface::Bus;

pub mod cache;
pub mod waitloop;

pub struct Optimizations<S: Bus> {
    pub cache: Cache<S>,
    pub waitloop: WaitloopData,
}

impl<S: Bus> Default for Optimizations<S> {
    fn default() -> Self {
        Self {
            cache: Default::default(),
            waitloop: Default::default(),
        }
    }
}
