pub trait NumExt {
    fn bit(self, bit: u16) -> u16;
    fn is_bit(&self, bit: u16) -> bool;
    fn set_bit(self, bit: u16, state: bool) -> u16;
    fn u16(self) -> u16;
    fn u8(self) -> u8;
    fn us(self) -> usize;
}

// TODO Maybe make a macro for these repeated invocations
impl NumExt for u8 {
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
    fn u16(self) -> u16 {
        self as u16
    }

    #[inline(always)]
    fn u8(self) -> u8 {
        self
    }

    #[inline(always)]
    fn us(self) -> usize {
        self as usize
    }
}

impl NumExt for u16 {
    #[inline(always)]
    fn bit(self, bit: u16) -> u16 {
        (self >> bit) & 1
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
    fn u16(self) -> u16 {
        self
    }

    #[inline(always)]
    fn u8(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    fn us(self) -> usize {
        self as usize
    }
}
