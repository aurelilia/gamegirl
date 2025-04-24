use core::marker::PhantomData;

use crate::interface::Bus;

#[derive(Default)]
pub struct Cache<S: Bus> {
    _s: PhantomData<S>,
}
