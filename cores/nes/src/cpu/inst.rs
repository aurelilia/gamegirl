// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use common::numutil::{hword, NumExt, U16Ext};

use super::{Cpu, CpuStatus, Reg, Reg::*};
use crate::Nes;

type AddressingMode = fn(&mut Nes) -> u16;

#[derive(Copy, Clone)]
pub struct Inst(pub u8);

pub fn execute(nes: &mut Nes, inst: Inst) {
    match inst.0 {
        //// Loads / Stores
        // LDA
        0xA9 => ld(A, nes.read_imm(), nes),
        0xA5 => ld(A, load(nes, addr::zero_page), nes),
        0xB5 => ld(A, load(nes, addr::zero_page_x), nes),
        0xAD => ld(A, load(nes, addr::absolute), nes),
        0xBD => ld(A, load(nes, addr::absolute_x), nes),
        0xB9 => ld(A, load(nes, addr::absolute_y), nes),
        0xA1 => ld(A, load(nes, addr::indirect_x), nes),
        0xB1 => ld(A, load(nes, addr::indirect_y), nes),

        // LDX
        0xA2 => ld(X, nes.read_imm(), nes),
        0xA6 => ld(X, load(nes, addr::zero_page), nes),
        0xB6 => ld(X, load(nes, addr::zero_page_y), nes),
        0xAE => ld(X, load(nes, addr::absolute), nes),
        0xBE => ld(X, load(nes, addr::absolute_y), nes),

        // LDY
        0xA0 => ld(Y, nes.read_imm(), nes),
        0xA4 => ld(Y, load(nes, addr::zero_page), nes),
        0xB4 => ld(Y, load(nes, addr::zero_page_x), nes),
        0xAC => ld(Y, load(nes, addr::absolute), nes),
        0xBC => ld(Y, load(nes, addr::absolute_x), nes),

        // STA
        0x85 => st(A, addr::zero_page, nes),
        0x95 => st(A, addr::zero_page_x, nes),
        0x8D => st(A, addr::absolute, nes),
        0x9D => st(A, addr::absolute_x, nes),
        0x99 => st(A, addr::absolute_y, nes),
        0x81 => st(A, addr::indirect_x, nes),
        0x91 => st(A, addr::indirect_y, nes),

        // STX
        0x86 => st(X, addr::zero_page, nes),
        0x96 => st(X, addr::zero_page_y, nes),
        0x8E => st(X, addr::absolute, nes),

        // STY
        0x84 => st(Y, addr::zero_page, nes),
        0x94 => st(Y, addr::zero_page_x, nes),
        0x8C => st(Y, addr::absolute, nes),

        //// Register Transfers
        0xAA => t(A, X, nes),
        0xA8 => t(A, Y, nes),
        0x8A => t(X, A, nes),
        0x98 => t(Y, A, nes),

        //// Stack Operations
        0xBA => t(S, X, nes),
        0x9A => txs(nes),
        0x48 => nes.push(nes.cpu.get(A)),                    // PHA
        0x08 => nes.push(nes.cpu.status.into()),             // PHP
        0x68 => pla(nes),                                    // PLA
        0x28 => nes.cpu.status = CpuStatus::from(nes.pop()), // PLP

        //// Logical
        // AND
        0x29 => and(nes.read_imm(), nes),
        0x25 => and(load(nes, addr::zero_page), nes),
        0x35 => and(load(nes, addr::zero_page_x), nes),
        0x2D => and(load(nes, addr::absolute), nes),
        0x3D => and(load(nes, addr::absolute_x), nes),
        0x39 => and(load(nes, addr::absolute_y), nes),
        0x21 => and(load(nes, addr::indirect_x), nes),
        0x31 => and(load(nes, addr::indirect_y), nes),

        // EOR
        0x49 => eor(nes.read_imm(), nes),
        0x45 => eor(load(nes, addr::zero_page), nes),
        0x55 => eor(load(nes, addr::zero_page_x), nes),
        0x4D => eor(load(nes, addr::absolute), nes),
        0x5D => eor(load(nes, addr::absolute_x), nes),
        0x59 => eor(load(nes, addr::absolute_y), nes),
        0x41 => eor(load(nes, addr::indirect_x), nes),
        0x51 => eor(load(nes, addr::indirect_y), nes),

        // ORA
        0x09 => ora(nes.read_imm(), nes),
        0x05 => ora(load(nes, addr::zero_page), nes),
        0x15 => ora(load(nes, addr::zero_page_x), nes),
        0x0D => ora(load(nes, addr::absolute), nes),
        0x1D => ora(load(nes, addr::absolute_x), nes),
        0x19 => ora(load(nes, addr::absolute_y), nes),
        0x01 => ora(load(nes, addr::indirect_x), nes),
        0x11 => ora(load(nes, addr::indirect_y), nes),

        // BIT
        0x24 => bit(load(nes, addr::zero_page), nes),
        0x2C => bit(load(nes, addr::absolute), nes),

        //// Arithmetic
        // ADC
        0x69 => adc(nes.read_imm(), nes),
        0x65 => adc(load(nes, addr::zero_page), nes),
        0x75 => adc(load(nes, addr::zero_page_x), nes),
        0x6D => adc(load(nes, addr::absolute), nes),
        0x7D => adc(load(nes, addr::absolute_x), nes),
        0x79 => adc(load(nes, addr::absolute_y), nes),
        0x61 => adc(load(nes, addr::indirect_x), nes),
        0x71 => adc(load(nes, addr::indirect_y), nes),

        // SBC
        0xE9 => sbc(nes.read_imm(), nes),
        0xE5 => sbc(load(nes, addr::zero_page), nes),
        0xF5 => sbc(load(nes, addr::zero_page_x), nes),
        0xED => sbc(load(nes, addr::absolute), nes),
        0xFD => sbc(load(nes, addr::absolute_x), nes),
        0xF9 => sbc(load(nes, addr::absolute_y), nes),
        0xE1 => sbc(load(nes, addr::indirect_x), nes),
        0xF1 => sbc(load(nes, addr::indirect_y), nes),

        // CMP
        0xC9 => cp(A, nes.read_imm(), nes),
        0xC5 => cp(A, load(nes, addr::zero_page), nes),
        0xD5 => cp(A, load(nes, addr::zero_page_x), nes),
        0xCD => cp(A, load(nes, addr::absolute), nes),
        0xDD => cp(A, load(nes, addr::absolute_x), nes),
        0xD9 => cp(A, load(nes, addr::absolute_y), nes),
        0xC1 => cp(A, load(nes, addr::indirect_x), nes),
        0xD1 => cp(A, load(nes, addr::indirect_y), nes),

        // CPX
        0xE0 => cp(X, nes.read_imm(), nes),
        0xE4 => cp(X, load(nes, addr::zero_page), nes),
        0xEC => cp(X, load(nes, addr::absolute), nes),
        // CPY
        0xC0 => cp(Y, nes.read_imm(), nes),
        0xC4 => cp(Y, load(nes, addr::zero_page), nes),
        0xCC => cp(Y, load(nes, addr::absolute), nes),

        //// Increments/Decrements
        // INC
        0xE6 => incdec(1, addr::zero_page, nes),
        0xF6 => incdec(1, addr::zero_page_x, nes),
        0xEE => incdec(1, addr::absolute, nes),
        0xFE => incdec(1, addr::absolute_x, nes),

        // DEC
        0xC6 => incdec(-1, addr::zero_page, nes),
        0xD6 => incdec(-1, addr::zero_page_x, nes),
        0xCE => incdec(-1, addr::absolute, nes),
        0xDE => incdec(-1, addr::absolute_x, nes),

        // Remaining
        0xE8 => inde(1, X, nes),
        0xC8 => inde(1, Y, nes),
        0xCA => inde(-1, X, nes),
        0x88 => inde(-1, Y, nes),

        //// Shifts
        // ASL
        0x0A => {
            let value = asl_inner(nes.cpu.get(A), nes);
            nes.cpu.set(A, value)
        }
        0x06 => asl(addr::zero_page, nes),
        0x16 => asl(addr::zero_page_x, nes),
        0x0E => asl(addr::absolute, nes),
        0x1E => asl(addr::absolute_x, nes),

        // LSR
        0x4A => {
            let value = lsr_inner(nes.cpu.get(A), nes);
            nes.cpu.set(A, value)
        }
        0x46 => lsr(addr::zero_page, nes),
        0x56 => lsr(addr::zero_page_x, nes),
        0x4E => lsr(addr::absolute, nes),
        0x5E => lsr(addr::absolute_x, nes),

        // ROL
        0x2A => {
            let value = rol_inner(nes.cpu.get(A), nes);
            nes.cpu.set(A, value)
        }
        0x26 => rol(addr::zero_page, nes),
        0x36 => rol(addr::zero_page_x, nes),
        0x2E => rol(addr::absolute, nes),
        0x3E => rol(addr::absolute_x, nes),

        // ROR
        0x6A => {
            let value = ror_inner(nes.cpu.get(A), nes);
            nes.cpu.set(A, value)
        }
        0x66 => ror(addr::zero_page, nes),
        0x76 => ror(addr::zero_page_x, nes),
        0x6E => ror(addr::absolute, nes),
        0x7E => ror(addr::absolute_x, nes),

        //// Jumps / Calls
        0x4C => jmp(addr::absolute, nes),
        0x6C => jmp(addr::indirect, nes),
        0x20 => jsr(nes),
        0x60 => rts(nes),

        //// Branches
        0x90 => br(false, nes.cpu.status.carry(), nes),
        0xB0 => br(true, nes.cpu.status.carry(), nes),
        0xD0 => br(false, nes.cpu.status.zero(), nes),
        0xF0 => br(true, nes.cpu.status.zero(), nes),
        0x10 => br(false, nes.cpu.status.negative(), nes),
        0x30 => br(true, nes.cpu.status.negative(), nes),
        0x50 => br(false, nes.cpu.status.overflow(), nes),
        0x70 => br(true, nes.cpu.status.overflow(), nes),

        //// Status Flag Changes
        0x18 => {
            nes.cpu.status.set_carry(false);
            nes.advance_clock(1);
        }
        0x38 => {
            nes.cpu.status.set_carry(true);
            nes.advance_clock(1);
        }
        0xD8 => {
            nes.cpu.status.set_demimal_mode(false);
            nes.advance_clock(1);
        }
        0xF8 => {
            nes.cpu.status.set_demimal_mode(true);
            nes.advance_clock(1);
        }
        0x58 => {
            nes.cpu.status.set_interrupt_disable(false);
            nes.advance_clock(1);
        }
        0x78 => {
            nes.cpu.status.set_interrupt_disable(true);
            nes.advance_clock(1);
        }
        0xB8 => {
            nes.cpu.status.set_overflow(false);
            nes.advance_clock(1);
        }

        //// System Functions
        0x00 => brk(nes),
        0xEA => nes.advance_clock(1), // NOP
        0x40 => rti(nes),
        _ => log::error!("Unknown instruction 0x{:X}!", inst.0),
    }
}

fn ld(reg: Reg, value: u8, nes: &mut Nes) {
    nes.cpu.set(reg, value);
    nes.cpu.status.set_zn(value);
}

fn load(nes: &mut Nes, mode: AddressingMode) -> u8 {
    let addr = mode(nes);
    nes.read(addr)
}

fn st(src: Reg, mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    nes.write(addr, nes.cpu.get(src));
}

fn t(src: Reg, dst: Reg, nes: &mut Nes) {
    nes.advance_clock(1);
    let value = nes.cpu.get(src);
    nes.cpu.set(dst, value);
    nes.cpu.status.set_zn(value);
}

fn txs(nes: &mut Nes) {
    nes.advance_clock(1);
    let value = nes.cpu.get(X);
    nes.cpu.set(S, value);
}

fn pla(nes: &mut Nes) {
    let value = nes.pop();
    nes.cpu.set(A, value);
    nes.cpu.status.set_zn(value);
}

fn and(other: u8, nes: &mut Nes) {
    let value = nes.cpu.get(A) & other;
    nes.cpu.set(A, value);
    nes.cpu.status.set_zn(value);
    nes.advance_clock(1);
}

fn eor(other: u8, nes: &mut Nes) {
    let value = nes.cpu.get(A) ^ other;
    nes.cpu.set(A, value);
    nes.cpu.status.set_zn(value);
    nes.advance_clock(1);
}

fn ora(other: u8, nes: &mut Nes) {
    let value = nes.cpu.get(A) | other;
    nes.cpu.set(A, value);
    nes.cpu.status.set_zn(value);
    nes.advance_clock(1);
}

fn bit(value: u8, nes: &mut Nes) {
    nes.cpu.status.set_zero(nes.cpu.get(A) | value == 0);
    nes.cpu.status.set_overflow(value.is_bit(6));
    nes.cpu.status.set_negative(value.is_bit(7));
    nes.advance_clock(1);
}

fn adc(value: u8, nes: &mut Nes) {
    let acc = nes.cpu.get(A).u16();
    let value = acc + value.u16() + nes.cpu.status.carry() as u16;
    nes.cpu.status.set_znc(value);
    nes.cpu.set(A, value.u8());
    nes.advance_clock(1);
}

fn sbc(value: u8, nes: &mut Nes) {
    let acc = nes.cpu.get(A).u16();
    let value = acc
        .wrapping_sub(value.u16())
        .wrapping_sub(!(nes.cpu.status.carry() as u16));
    nes.cpu.status.set_znc(value);
    nes.cpu.status.set_carry(!nes.cpu.status.carry());
    nes.cpu.set(A, value.u8());
    nes.advance_clock(1);
}

fn cp(reg: Reg, other: u8, nes: &mut Nes) {
    let acc = nes.cpu.get(reg);
    let value = acc.wrapping_sub(other);
    nes.cpu.status.set_zn(value);
    nes.cpu.status.set_carry(acc >= other);
    nes.advance_clock(1);
}

fn incdec(value: i8, mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    let value = nes.read(addr).wrapping_add_signed(value);
    nes.advance_clock(1);
    nes.write(addr, value);
    nes.cpu.status.set_zn(value);
}

fn inde(value: i8, reg: Reg, nes: &mut Nes) {
    let value = nes.cpu.get(reg).wrapping_add_signed(value);
    nes.advance_clock(1);
    nes.cpu.set(reg, value);
    nes.cpu.status.set_zn(value);
}

fn asl(mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    let old_value = nes.read(addr);
    let value = asl_inner(old_value, nes);
    nes.write(addr, value);
}

fn asl_inner(old_value: u8, nes: &mut Nes) -> u8 {
    let value = old_value << 1;
    nes.cpu.status.set_zn(value);
    nes.cpu.status.set_carry(old_value.is_bit(7));
    nes.advance_clock(1);
    value
}

fn lsr(mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    let old_value = nes.read(addr);
    let value = lsr_inner(old_value, nes);
    nes.write(addr, value);
}

fn lsr_inner(old_value: u8, nes: &mut Nes) -> u8 {
    let value = old_value >> 1;
    nes.cpu.status.set_zn(value);
    nes.cpu.status.set_carry(old_value.is_bit(0));
    nes.advance_clock(1);
    value
}

fn rol(mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    let old_value = nes.read(addr);
    let value = rol_inner(old_value, nes);
    nes.write(addr, value);
}

fn rol_inner(old_value: u8, nes: &mut Nes) -> u8 {
    let value = (old_value << 1).set_bit(0, nes.cpu.status.carry());
    nes.cpu.status.set_zn(value);
    nes.cpu.status.set_carry(old_value.is_bit(7));
    nes.advance_clock(1);
    value
}

fn ror(mode: AddressingMode, nes: &mut Nes) {
    let addr = mode(nes);
    let old_value = nes.read(addr);
    let value = ror_inner(old_value, nes);
    nes.write(addr, value);
}

fn ror_inner(old_value: u8, nes: &mut Nes) -> u8 {
    let value = (old_value >> 1).set_bit(7, nes.cpu.status.carry());
    nes.cpu.status.set_zn(value);
    nes.cpu.status.set_carry(old_value.is_bit(0));
    nes.advance_clock(1);
    value
}

fn jmp(mode: AddressingMode, nes: &mut Nes) {
    nes.cpu.pc = mode(nes);
}

fn jsr(nes: &mut Nes) {
    nes.push(nes.cpu.pc.low());
    nes.push(nes.cpu.pc.high());
    jmp(addr::absolute, nes);
}

fn rts(nes: &mut Nes) {
    let hi = nes.pop();
    let lo = nes.pop();
    nes.cpu.pc = hword(lo, hi);
}

fn br(want: bool, is: bool, nes: &mut Nes) {
    let addr = nes.read_imm() as i8;
    if want == is {
        nes.advance_clock(1);
        nes.cpu.pc = nes.cpu.pc.wrapping_add_signed(addr as i16);
    }
}

fn brk(nes: &mut Nes) {
    Cpu::trigger_int(nes);
}

fn rti(nes: &mut Nes) {
    nes.cpu.status = CpuStatus::from(nes.pop());
    let hi = nes.pop();
    let lo = nes.pop();
    nes.cpu.pc = hword(lo, hi);
}

mod addr {
    use common::numutil::hword;

    use super::*;

    pub fn zero_page(nes: &mut Nes) -> u16 {
        nes.read_imm().u16()
    }

    pub fn zero_page_x(nes: &mut Nes) -> u16 {
        let zero_addr = nes.read_imm();
        nes.advance_clock(1);
        zero_addr.wrapping_add(nes.cpu.get(X)).u16()
    }

    pub fn zero_page_y(nes: &mut Nes) -> u16 {
        let zero_addr = nes.read_imm();
        nes.advance_clock(1);
        zero_addr.wrapping_add(nes.cpu.get(Y)).u16()
    }

    pub fn absolute(nes: &mut Nes) -> u16 {
        hword(nes.read_imm(), nes.read_imm())
    }

    pub fn absolute_x(nes: &mut Nes) -> u16 {
        let abs = absolute(nes);
        abs.wrapping_add(nes.cpu.get(X).u16())
    }

    pub fn absolute_y(nes: &mut Nes) -> u16 {
        let abs = absolute(nes);
        abs.wrapping_add(nes.cpu.get(Y).u16())
    }

    pub fn indirect(nes: &mut Nes) -> u16 {
        let addr = absolute(nes);
        hword(nes.read(addr), nes.read(addr + 1))
    }

    pub fn indirect_x(nes: &mut Nes) -> u16 {
        let imm = nes.read_imm().wrapping_add(nes.cpu.get(X));
        hword(nes.read(imm.u16()), nes.read(imm.wrapping_add(1).u16()))
    }

    pub fn indirect_y(nes: &mut Nes) -> u16 {
        let imm = nes.read_imm();
        let addr = hword(nes.read(imm.u16()), nes.read(imm.wrapping_add(1).u16()));
        addr.wrapping_add(nes.cpu.get(Y).u16())
    }
}
