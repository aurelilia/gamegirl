pub trait NumExt {
    fn bit(self, bit: u16) -> u16;
    fn is_bit(&self, bit: u16) -> bool;
    fn en_bit(self, bit: u16) -> u16;
    fn set_bit(self, bit: u16, state: bool) -> u16;
    fn u16(self) -> u16;
    fn u8(self) -> u8;
}

// TODO Maybe make a macro for these repeated invocations
impl NumExt for u8 {
    fn bit(self, bit: u16) -> u16 {
        (self.u16() & (1 << bit)) >> bit
    }

    fn is_bit(&self, bit: u16) -> bool {
        (self & (1 << bit)) != 0
    }

    fn en_bit(self, bit: u16) -> u16 {
        (self.u16() & ((1 << bit) ^ 0xFF)) | (1 << bit)
    }

    fn set_bit(self, bit: u16, state: bool) -> u16 {
        (self.u16() & ((1 << bit) ^ 0xFF)) | ((state as u16) << bit)
    }

    fn u16(self) -> u16 {
        self as u16
    }

    fn u8(self) -> u8 {
        self
    }
}

impl NumExt for u16 {
    fn bit(self, bit: u16) -> u16 {
        (self.u16() & (1 << bit)) >> bit
    }

    fn is_bit(&self, bit: u16) -> bool {
        (self & (1 << bit)) != 0
    }

    fn en_bit(self, bit: u16) -> u16 {
        (self.u16() & ((1 << bit) ^ 0xFF)) | (1 << bit)
    }

    fn set_bit(self, bit: u16, state: bool) -> u16 {
        (self.u16() & ((1 << bit) ^ 0xFF)) | ((state as u16) << bit)
    }

    fn u16(self) -> u16 {
        self
    }

    fn u8(self) -> u8 {
        self as u8
    }
}
