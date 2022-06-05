/// Trait for common number operations.
pub trait NumExt {
    /// Get the state of the given bit. Returns 0/1.
    fn bit(self, bit: u16) -> u16;
    /// Is the given bit set?
    fn is_bit(&self, bit: u16) -> bool;
    /// Set the given bit.
    fn set_bit(self, bit: u16, state: bool) -> u16;
    /// Convert to u8
    fn u8(self) -> u8;
    /// Convert to u16
    fn u16(self) -> u16;
    /// Convert to usize
    fn us(self) -> usize;
}

macro_rules! num_ext_impl {
    ($ty:ident) => {
        impl NumExt for $ty {
            #[inline(always)]
            fn bit(self, bit: u16) -> u16 {
                ((self >> bit) & 1).u16()
            }

            #[inline(always)]
            fn is_bit(&self, bit: u16) -> bool {
                (self & (1 << bit)) != 0
            }

            #[inline(always)]
            fn set_bit(self, bit: u16, state: bool) -> u16 {
                (self.u16() & ((1 << bit) ^ 0xFF)) | ((state as u16) << bit)
            }

            #[inline(always)]
            fn u8(self) -> u8 {
                self as u8
            }

            #[inline(always)]
            fn u16(self) -> u16 {
                self as u16
            }

            #[inline(always)]
            fn us(self) -> usize {
                self as usize
            }
        }
    };
}

num_ext_impl!(u8);
num_ext_impl!(u16);
num_ext_impl!(usize);
