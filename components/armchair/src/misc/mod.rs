use alloc::{fmt::Debug, format, string::String};

use crate::Address;

mod alu;
mod operations;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstructionKind {
    Arm = 0,
    Thumb = 1,
}

impl InstructionKind {
    /// Returns the width of an instruction in bytes.
    pub fn width(self) -> u32 {
        4 - (((self == Self::Thumb) as u32) << 1)
    }

    /// Returns the width of an instruction expressed as an address.
    pub fn addr_width(self) -> Address {
        Address(self.width())
    }
}

pub fn condition_mnemonic(cond: u16) -> &'static str {
    match cond {
        0x0 => "eq",
        0x1 => "ne",
        0x2 => "cs",
        0x3 => "cc",
        0x4 => "mi",
        0x5 => "pl",
        0x6 => "vs",
        0x7 => "vc",
        0x8 => "hi",
        0x9 => "ls",
        0xA => "ge",
        0xB => "lt",
        0xC => "gt",
        0xD => "le",
        0xE => "",
        _ => "nv",
    }
}

pub fn print_op<T: Debug>(item: T) -> String {
    format!("{item:?}").to_lowercase()
}
