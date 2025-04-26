use core::marker::PhantomData;

use crate::interface::Bus;

pub struct Cache<S: Bus> {
    _s: PhantomData<S>,
}

impl<S: Bus> Cache<S> {
    /// Invalidate all caches in the given page.
    pub fn invalidate_address(&mut self, pc: u32) {}
}

impl<S: Bus> Default for Cache<S> {
    fn default() -> Self {
        Self {
            _s: Default::default(),
        }
    }
}
