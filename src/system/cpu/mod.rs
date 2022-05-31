use crate::numutil::NumExt;
use crate::system::io::addr::{IE, IF};
use crate::system::GameGirl;

mod alu;
mod data;
mod inst;

#[derive(Debug, Default)]
pub struct Cpu {
    pub pc: u16,
    pub sp: u16,
    pub ime: bool,
    pub halt: bool,
    halt_bug: bool,
    regs: [u8; 8],
}

impl Cpu {
    pub(super) fn exec_next_inst(gg: &mut GameGirl) {
        let ime = gg.cpu.ime;

        if gg.cpu.halt {
            gg.advance_clock(1);
            gg.cpu.halt = (gg.mmu[IF] & gg.mmu[IE] & 0x1F) == 0
        } else {
            let inst = inst::get_next(gg);
            gg.advance_clock(inst.pre_cycles() as usize);

            if gg.cpu.halt_bug {
                gg.cpu.pc -= 1;
                gg.cpu.halt_bug = false;
            }

            let (cycles, inc) = inst::execute(gg, inst);
            if inc {
                gg.cpu.pc += inst.size().u16();
            }
            gg.advance_clock(cycles as usize);
        }

        if Self::check_interrupts(gg, ime && gg.cpu.ime) {
            gg.advance_clock(5)
        }
    }

    fn check_interrupts(gg: &mut GameGirl, ime: bool) -> bool {
        let bits = gg.mmu[IE] & gg.mmu[IF];
        if !ime || (bits == 0) {
            return false;
        }

        for bit in 0..5 {
            if bits.is_bit(bit) {
                gg.cpu.halt = false;
                gg.mmu[IF] = gg.mmu[IF].set_bit(bit, false) as u8;
                gg.cpu.ime = false;
                gg.push_stack(gg.cpu.pc);
                gg.cpu.pc = Interrupt::from_index(bit).addr();
                return true;
            }
        }
        false
    }

    pub fn flag(&self, flag: Flag) -> bool {
        self.reg(Reg::F).is_bit(flag.bit())
    }

    pub fn set_fli(&mut self, flag: Flag, val: u16) {
        self.set_fl(flag, val != 0)
    }

    pub fn set_fl(&mut self, flag: Flag, val: bool) {
        self.regs[Reg::F.i()] = self.reg(Reg::F).set_bit(flag.bit(), val).u8()
    }

    pub fn reg(&self, reg: Reg) -> u8 {
        self.regs[reg.i()]
    }

    pub fn set_reg(&mut self, reg: Reg, value: u8) {
        // Register F only allows writing the 4 high/flag bits
        let value = if reg == Reg::F { value & 0xF0 } else { value };
        self.regs[reg.i()] = value
    }

    pub fn dreg(&self, reg: DReg) -> u16 {
        let low = self.reg(reg.low());
        let high = self.reg(reg.high());
        (high.u16() << 8) | low.u16()
    }

    fn set_dreg(&mut self, reg: DReg, value: u16) {
        self.set_reg(reg.low(), value.u8());
        self.set_reg(reg.high(), (value >> 8).u8());
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Reg {
    A,
    B,
    C,
    D,
    E,
    F,
    H,
    L,
}

impl Reg {
    pub fn i(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DReg {
    BC,
    DE,
    HL,
    AF,
}

impl DReg {
    pub fn low(self) -> Reg {
        match self {
            DReg::BC => Reg::C,
            DReg::DE => Reg::E,
            DReg::HL => Reg::L,
            DReg::AF => Reg::F,
        }
    }

    pub fn high(self) -> Reg {
        match self {
            DReg::BC => Reg::B,
            DReg::DE => Reg::D,
            DReg::HL => Reg::H,
            DReg::AF => Reg::A,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Flag {
    Zero = 7,
    Negative = 6,
    HalfCarry = 5,
    Carry = 4,
}

impl Flag {
    pub fn bit(self) -> u16 {
        self as u16
    }

    pub fn mask(self) -> u8 {
        1 << self as u8
    }

    pub fn from(self, value: u16) -> u8 {
        if value != 0 {
            self.mask().u8()
        } else {
            0
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Interrupt {
    VBlank = 0x0040,
    Stat = 0x0048,
    Timer = 0x0050,
    Serial = 0x0058,
    Joypad = 0x0060,
}

impl Interrupt {
    const ORDER: [Interrupt; 5] = [
        Self::VBlank,
        Self::Stat,
        Self::Timer,
        Self::Serial,
        Self::Joypad,
    ];

    pub fn to_index(self) -> u16 {
        (self.addr() - 0x40) / 8
    }

    pub fn from_index(index: u16) -> Self {
        Self::ORDER[index as usize]
    }

    pub fn addr(self) -> u16 {
        self as u16
    }
}
