// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use bitmatch::bitmatch;
use common::numutil::{NumExt, U32Ext};

use crate::{
    cpu::{Cpu, Exception, PendingLoad},
    PlayStation,
};

macro_rules! check_cache {
    ($me:ident) => {
        if $me.cache_isolated() {
            return;
        }
    };
}

pub type InstructionHandler = fn(ps: &mut PlayStation, inst: Inst);
type Lut = [InstructionHandler; 64];

const PRIMARY: Lut = PlayStation::primary_table();
const SECONDARY: Lut = PlayStation::secondary_table();

impl PlayStation {
    pub fn run_inst(&mut self, inst: u32) {
        let primary = inst.bits(26, 6);
        let handler = PRIMARY[primary.us()];
        handler(self, Inst(inst));
    }

    const fn primary_table() -> Lut {
        let mut lut: Lut = [Self::unknown_instruction; 64];
        lut[0x00] = Self::secondary;
        lut[0x01] = Self::bcondz;
        lut[0x02] = Self::j;
        lut[0x03] = Self::jal;
        lut[0x04] = Self::beq;
        lut[0x05] = Self::bne;
        lut[0x06] = Self::blez;
        lut[0x07] = Self::bgtz;

        lut[0x08] = Self::math_signed::<true, "ADD">; // ADDI
        lut[0x09] = Self::math_signed::<true, "ADDU">; // ADDIU
        lut[0x0A] = Self::math_signed::<true, "SLT">; // SLTI
        lut[0x0B] = Self::math_unsigned::<true, "SLT">; // SLTIU
        lut[0x0C] = Self::math_unsigned::<true, "AND">; // ANDI
        lut[0x0D] = Self::math_unsigned::<true, "OR">; // ORI
        lut[0x0E] = Self::math_unsigned::<true, "XOR">; // XORI
        lut[0x0F] = Self::math_unsigned::<true, "LUI">; // LUI

        lut[0x10] = Self::cop0;
        lut[0x11] = Self::exception_inst::<{ Exception::CopError }>; // COP1
        lut[0x12] = Self::cop2;
        lut[0x13] = Self::exception_inst::<{ Exception::CopError }>; // COP3

        lut[0x20] = Self::load::<u8, true>; // LB
        lut[0x21] = Self::load::<u16, true>; // LH
        lut[0x22] = Self::lwl;
        lut[0x23] = Self::load::<u32, true>; // LW
        lut[0x24] = Self::load::<u8, false>; // LBU
        lut[0x25] = Self::load::<u16, false>; // LHU
        lut[0x26] = Self::lwr;

        lut[0x28] = Self::store::<1>; // SB
        lut[0x29] = Self::store::<2>; // SH
        lut[0x2A] = Self::swl;
        lut[0x2B] = Self::store::<4>; // SW
        lut[0x2E] = Self::swr;

        lut[0x30] = Self::exception_inst::<{ Exception::CopError }>; // LWC0
        lut[0x31] = Self::exception_inst::<{ Exception::CopError }>; // LWC1
        lut[0x32] = Self::lwc2;
        lut[0x33] = Self::exception_inst::<{ Exception::CopError }>; // LWC3
        lut[0x38] = Self::exception_inst::<{ Exception::CopError }>; // SWC0
        lut[0x39] = Self::exception_inst::<{ Exception::CopError }>; // SWC1
        lut[0x3A] = Self::swc2;
        lut[0x3B] = Self::exception_inst::<{ Exception::CopError }>; // SWC3

        lut
    }

    const fn secondary_table() -> Lut {
        let mut lut: Lut = [Self::unknown_instruction; 64];
        lut[0x00] = Self::shift::<true, "SLL">; // SLL
        lut[0x02] = Self::shift::<true, "SRL">; // SRL
        lut[0x03] = Self::shift::<true, "SRA">; // SRA
        lut[0x04] = Self::shift::<false, "SLL">; // SLLV
        lut[0x06] = Self::shift::<false, "SRL">; // SRLV
        lut[0x07] = Self::shift::<false, "SRA">; // SRAV

        lut[0x08] = Self::jr;
        lut[0x09] = Self::jalr;
        lut[0x0C] = Self::exception_inst::<{ Exception::Syscall }>; // SYSCALL
        lut[0x0D] = Self::exception_inst::<{ Exception::Break }>; // BREAK

        lut[0x10] = Self::lohi_mov::<false, false>; // MTLO
        lut[0x11] = Self::lohi_mov::<false, true>; // MTHI
        lut[0x12] = Self::lohi_mov::<true, false>; // MFLO
        lut[0x13] = Self::lohi_mov::<true, true>; // MFHI
        lut[0x18] = Self::muldiv::<false, true>; // MULT
        lut[0x19] = Self::muldiv::<false, false>; // MULTU
        lut[0x1A] = Self::muldiv::<true, true>; // DIV
        lut[0x1B] = Self::muldiv::<true, false>; // DIVU

        lut[0x20] = Self::math_signed::<false, "ADD">; // ADD
        lut[0x21] = Self::math_signed::<false, "ADDU">; // ADDU
        lut[0x22] = Self::math_signed::<false, "SUB">; // SUB
        lut[0x23] = Self::math_signed::<false, "SUBU">; // SUBU
        lut[0x24] = Self::math_unsigned::<false, "AND">; // AND
        lut[0x25] = Self::math_unsigned::<false, "OR">; // OR
        lut[0x26] = Self::math_unsigned::<false, "XOR">; // XOR
        lut[0x27] = Self::math_unsigned::<false, "NOR">; // NOR
        lut[0x2A] = Self::math_signed::<false, "SLT">; // SLT
        lut[0x2B] = Self::math_unsigned::<false, "SLT">; // SLTU

        lut
    }
}

// Utility
impl PlayStation {
    fn branch(&mut self, imm: i32) {
        // Always word-aligned
        let offs = imm << 2;
        self.jump_pc(self.cpu.pc.wrapping_add_signed(offs));
    }

    fn cache_isolated(&self) -> bool {
        self.cpu.cop0.sr.is_bit(16)
    }

    fn addr_with_imm(&self, inst: Inst) -> u32 {
        self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s())
    }

    fn exception_inst<const EX: Exception>(&mut self, _inst: Inst) {
        Cpu::exception_occurred(self, EX);
    }

    fn math_unsigned<const IMM: bool, const OP: &'static str>(&mut self, inst: Inst) {
        let a = self.cpu.reg(inst.rs());
        let b = if IMM {
            inst.imm16()
        } else {
            self.cpu.reg(inst.rt())
        };

        let value = match OP {
            "AND" => a & b,
            "OR" => a | b,
            "XOR" => a ^ b,
            "LUI" => b << 16,
            "SLT" => (a < b) as u32,
            "NOR" => !(a | b),
            t => panic!("Unknown math operation {t}"),
        };

        let reg = if IMM { inst.rt() } else { inst.rd() };
        self.cpu.set_reg(reg, value);
    }

    fn math_signed<const IMM: bool, const OP: &'static str>(&mut self, inst: Inst) {
        let a = self.cpu.reg(inst.rs()) as i32;
        let b = if IMM {
            inst.imm16s()
        } else {
            self.cpu.reg(inst.rt()) as i32
        };

        let value = match OP {
            "ADDU" => a.wrapping_add(b),
            "SUBU" => a.wrapping_sub(b),
            "ADD" => match a.checked_add(b) {
                Some(value) => value,
                None => {
                    Cpu::exception_occurred(self, Exception::Overflow);
                    return;
                }
            },
            "SUB" => match a.checked_sub(b) {
                Some(value) => value,
                None => {
                    Cpu::exception_occurred(self, Exception::Overflow);
                    return;
                }
            },
            "SLT" => (a < b) as i32,
            t => panic!("Unknown math operation {t}"),
        };

        let reg = if IMM { inst.rt() } else { inst.rd() };
        self.cpu.set_reg(reg, value as u32);
    }

    fn unknown_instruction(&mut self, inst: Inst) {
        log::warn!("Unknown opcode 0x{:08X}", inst.0);
        self.exception_inst::<{ Exception::UnknownOpcode }>(inst);
    }
}

// Primary
impl PlayStation {
    fn bcondz(&mut self, inst: Inst) {
        let is_ge = inst.0.is_bit(16);
        let link = inst.0.bits(17, 4) == 0x8;

        let cond = (self.cpu.reg(inst.rs()) as i32) < 0;
        let cond = cond != is_ge;

        if link {
            self.cpu.set_reg(31, self.cpu.next_pc);
        }
        if cond {
            self.branch(inst.imm16s());
        }
    }

    fn j(&mut self, inst: Inst) {
        let pc = (self.cpu.pc & 0xF000_0000) | (inst.imm26() << 2);
        self.jump_pc(pc);
    }

    fn jal(&mut self, inst: Inst) {
        self.cpu.set_reg(31, self.cpu.next_pc);
        self.j(inst);
    }

    fn beq(&mut self, inst: Inst) {
        if self.cpu.reg(inst.rs()) == self.cpu.reg(inst.rt()) {
            self.branch(inst.imm16s());
        }
    }

    fn bne(&mut self, inst: Inst) {
        if self.cpu.reg(inst.rs()) != self.cpu.reg(inst.rt()) {
            self.branch(inst.imm16s());
        }
    }

    fn blez(&mut self, inst: Inst) {
        if (self.cpu.reg(inst.rs()) as i32) <= 0 {
            self.branch(inst.imm16s());
        }
    }

    fn bgtz(&mut self, inst: Inst) {
        if self.cpu.reg(inst.rs()) as i32 > 0 {
            self.branch(inst.imm16s());
        }
    }

    fn cop2(&mut self, inst: Inst) {
        todo!();
    }

    fn load<T: NumExt, const SIGN: bool>(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        Cpu::ensure_aligned(self, addr, T::WIDTH.u32(), Exception::UnalignedLoad);

        let value = match (T::WIDTH, SIGN) {
            (1, false) => self.get::<u8>(addr).u32(),
            (1, true) => self.get::<u8>(addr) as i8 as i32 as u32,
            (2, false) => self.get::<u16>(addr).u32(),
            (2, true) => self.get::<u16>(addr) as i16 as i32 as u32,
            (4, _) => self.get::<u32>(addr).u32(),
            _ => panic!("Invalid load parameters"),
        };
        self.cpu.pending_load = PendingLoad {
            reg: inst.rt(),
            value,
        };
    }

    fn lwr(&mut self, inst: Inst) {
        let addr = self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s());
        let value = self.cpu.next_regs[inst.rt().us()];

        let mem_aligned = self.get(addr & !3);
        let value = match addr & 3 {
            0 => mem_aligned,
            1 => (value & 0xFF00_0000) | (mem_aligned >> 8),
            2 => (value & 0xFFFF_0000) | (mem_aligned >> 16),
            _ => (value & 0xFFFF_FF00) | (mem_aligned >> 24),
        };
        self.cpu.pending_load = PendingLoad {
            reg: inst.rt(),
            value,
        };
    }

    fn lwl(&mut self, inst: Inst) {
        let addr = self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s());
        let value = self.cpu.next_regs[inst.rt().us()];

        let mem_aligned = self.get(addr & !3);
        let value = match addr & 3 {
            0 => (value & 0x00FF_FFFF) | (mem_aligned << 24),
            1 => (value & 0x0000_FFFF) | (mem_aligned << 16),
            2 => (value & 0x0000_00FF) | (mem_aligned << 8),
            _ => mem_aligned,
        };
        self.cpu.pending_load = PendingLoad {
            reg: inst.rt(),
            value,
        };
    }

    fn store<const SIZE: u8>(&mut self, inst: Inst) {
        check_cache!(self);
        let addr = self.addr_with_imm(inst);
        Cpu::ensure_aligned(self, addr, SIZE.u32(), Exception::UnalignedStore);

        let value = self.cpu.reg(inst.rt());
        match SIZE {
            1 => self.set(addr, value.u8()),
            2 => self.set(addr, value.u16()),
            4 => self.set(addr, value.u32()),
            _ => panic!("Invalid store parameters"),
        };
    }

    fn swl(&mut self, inst: Inst) {
        let addr = self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s());
        let value = self.cpu.reg(inst.rt());

        let mem_aligned = self.get::<u32>(addr & !3);
        let value = match addr & 3 {
            0 => (value & 0xFFFF_FF00) | (mem_aligned >> 24),
            1 => (value & 0xFFFF_0000) | (mem_aligned >> 16),
            2 => (value & 0xFF00_0000) | (mem_aligned >> 8),
            _ => mem_aligned,
        };
        self.set(addr & !3, value);
    }

    fn swr(&mut self, inst: Inst) {
        let addr = self.cpu.reg(inst.rs()).wrapping_add_signed(inst.imm16s());
        let value = self.cpu.reg(inst.rt());

        let mem_aligned = self.get::<u32>(addr & !3);
        let value = match addr & 3 {
            0 => mem_aligned,
            1 => (value & 0x0000_00FF) | (mem_aligned << 8),
            2 => (value & 0x0000_FFFF) | (mem_aligned << 16),
            _ => (value & 0x00FF_FFFF) | (mem_aligned << 24),
        };
        self.set(addr & !3, value);
    }

    fn lwc2(&mut self, inst: Inst) {
        todo!();
    }

    fn swc2(&mut self, inst: Inst) {
        todo!();
    }
}

// Secondary
impl PlayStation {
    fn secondary(&mut self, inst: Inst) {
        let secondary = inst.0.bits(0, 6);
        let handler = SECONDARY[secondary.us()];
        handler(self, inst);
    }

    fn shift<const IMM: bool, const OP: &'static str>(&mut self, inst: Inst) {
        let a = self.cpu.reg(inst.rt());
        let b = if IMM {
            inst.imm5()
        } else {
            self.cpu.reg(inst.rs()) & 0x1F
        };

        let value = match OP {
            "SLL" => a << b,
            "SRL" => a >> b,
            "SRA" => ((a as i32) >> b) as u32,
            _ => panic!("Unknown shift operation"),
        };

        self.cpu.set_reg(inst.rd(), value);
    }

    fn jr(&mut self, inst: Inst) {
        self.jump_pc(self.cpu.reg(inst.rs()));
    }

    fn jalr(&mut self, inst: Inst) {
        self.cpu.set_reg(inst.rd(), self.cpu.pc);
        self.jump_pc(self.cpu.reg(inst.rs()));
    }

    fn lohi_mov<const TO_REG: bool, const HI: bool>(&mut self, inst: Inst) {
        match (TO_REG, HI) {
            (true, true) => self.cpu.set_reg(inst.rd(), self.cpu.hi),
            (true, false) => self.cpu.set_reg(inst.rd(), self.cpu.lo),
            (false, true) => self.cpu.hi = self.cpu.reg(inst.rs()),
            (false, false) => self.cpu.lo = self.cpu.reg(inst.rs()),
        }
    }

    fn muldiv<const DIV: bool, const SIGN: bool>(&mut self, inst: Inst) {
        let a = self.cpu.reg(inst.rs()) as u64;
        let b = self.cpu.reg(inst.rt()) as u64;

        (self.cpu.lo, self.cpu.hi) = match (DIV, SIGN) {
            (false, false) => {
                let res = a.wrapping_mul(b);
                (res as u32, (res >> 32) as u32)
            }

            (false, true) => {
                let res = (a as i64).wrapping_mul(b as i64) as u64;
                (res as u32, (res >> 32) as u32)
            }

            (true, false) if b == 0 => (u32::MAX, a as u32),
            (true, false) => (a.wrapping_div(b) as u32, (a % b) as u32),

            (true, true) if b == 0 => {
                if (a as i32) < 0 {
                    (u32::MAX, a as u32)
                } else {
                    (1, a as u32)
                }
            }
            (true, true) if a == 0x8000_0000 && b as u32 == u32::MAX => (a as u32, 0),
            (true, true) => {
                let a = a as i64;
                let b = b as i64;
                (a.wrapping_div(b) as u32, (a % b) as u32)
            }
        };
    }
}

impl PlayStation {
    #[bitmatch]
    pub fn get_mnemonic(inst: u32) -> String {
        #[bitmatch]
        match inst {
            "000000_sssss_ttttt_ddddd_mmmmm_000000" => format!("sll r{d}, r{t}, {m}"),
            "000000_sssss_ttttt_ddddd_mmmmm_000010" => format!("srl r{d}, r{t}, {m}"),
            "000000_sssss_ttttt_ddddd_mmmmm_000011" => format!("sra r{d}, r{t}, {m}"),
            "000000_sssss_ttttt_ddddd_mmmmm_000100" => format!("sll r{d}, r{t}, r{s}"),
            "000000_sssss_ttttt_ddddd_mmmmm_000110" => format!("srl r{d}, r{t}, r{s}"),
            "000000_sssss_ttttt_ddddd_mmmmm_000111" => format!("sra r{d}, r{t}, r{s}"),

            "000000_sssss_ttttt_ddddd_mmmmm_001000" => format!("jr r{s}"),
            "000000_sssss_ttttt_ddddd_mmmmm_001001" => format!("jalr r{s} (r{d})"),
            "000000_sssss_sssss_sssss_sssss_001100" => format!("sys 0x{s:X}"),
            "000000_sssss_sssss_sssss_sssss_001101" => format!("break 0x{s:X}"),

            "000000_sssss_ttttt_ddddd_mmmmm_010000" => format!("mfhi r{d}"),
            "000000_sssss_ttttt_ddddd_mmmmm_010001" => format!("mthi r{d}"),
            "000000_sssss_ttttt_ddddd_mmmmm_010010" => format!("mflo r{d}"),
            "000000_sssss_ttttt_ddddd_mmmmm_010011" => format!("mtlo r{d}"),

            "000000_sssss_ttttt_ddddd_mmmmm_011000" => format!("mult r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_011001" => format!("multu r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_011010" => format!("div r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_011011" => format!("divu r{s}, r{t}"),

            "000000_sssss_ttttt_ddddd_mmmmm_100000" => format!("add r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100001" => format!("addu r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100010" => format!("sub r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100011" => format!("subu r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100100" => format!("and r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100101" => format!("or r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100110" => format!("xor r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_100111" => format!("nor r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_101010" => format!("slt r{d}, r{s}, r{t}"),
            "000000_sssss_ttttt_ddddd_mmmmm_101011" => format!("sltu r{d}, r{s}, r{t}"),

            "000001_sssss_0zzz0_mmmmm_mmmmm_mmmmmm" => format!("bltz r{s}, 0x{m:X}"),
            "000001_sssss_0zzz1_mmmmm_mmmmm_mmmmmm" => format!("bgez r{s}, 0x{m:X}"),
            "000001_sssss_10000_mmmmm_mmmmm_mmmmmm" => format!("bltzal r{s}, 0x{m:X}"),
            "000001_sssss_10001_mmmmm_mmmmm_mmmmmm" => format!("bgezal r{s}, 0x{m:X}"),

            "000010_mmmmm_mmmmm_mmmmm_mmmmm_mmmmmm" => format!("j 0x{m:X}"),
            "000011_mmmmm_mmmmm_mmmmm_mmmmm_mmmmmm" => format!("jal 0x{m:X}"),

            "000100_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("beq r{s}, r{t}, 0x{m:X}"),
            "000101_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("bne r{s}, r{t}, 0x{m:X}"),
            "000110_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("blez r{s}, 0x{m:X}"),
            "000111_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("bgtz r{s}, 0x{m:X}"),

            "001000_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("add r{t}, r{s}, 0x{m:X}"),
            "001001_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("addu r{t}, r{s}, 0x{m:X}"),
            "001010_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("slt r{t}, r{s}, 0x{m:X}"),
            "001011_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("sltu r{t}, r{s}, 0x{m:X}"),
            "001100_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("and r{t}, r{s}, 0x{m:X}"),
            "001101_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("or r{t}, r{s}, 0x{m:X}"),
            "001110_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("xor r{t}, r{s}, 0x{m:X}"),
            "001111_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lui r{t}, 0x{m:X}"),

            "0100nn_00000_ttttt_ddddd_mmmmm_000000" => format!("mfc{n} r{t}, r{d}"),
            "0100nn_00010_ttttt_ddddd_mmmmm_000000" => format!("cfc{n} r{t}, r{d}"),
            "0100nn_00100_ttttt_ddddd_mmmmm_000000" => format!("mtc{n} r{t}, r{d}"),
            "0100nn_00110_ttttt_ddddd_mmmmm_000000" => format!("ctc{n} r{t}, r{d}"),
            "0100nn_01000_00000_mmmmm_mmmmm_mmmmmm" => format!("bc{n}f 0x{m}"),
            "0100nn_01000_00001_mmmmm_mmmmm_mmmmmm" => format!("bc{n}t 0x{m}"),
            "0100nn_1mmmm_mmmmm_mmmmm_mmmmm_mmmmmm" => format!("cop{n} 0x{m}"),

            "1100nn_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lwc{n} r{t}, [r{s}+0x{m:X}]"),
            "1110nn_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("swc{n} r{t}, [r{s}+0x{m:X}]"),

            "100000_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lb r{t}, [r{s}+0x{m:X}]"),
            "100001_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lh r{t}, [r{s}+0x{m:X}]"),
            "100010_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lwl r{t}, [r{s}+0x{m:X}]"),
            "100011_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lw r{t}, [r{s}+0x{m:X}]"),
            "100100_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lbu r{t}, [r{s}+0x{m:X}]"),
            "100101_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lhu r{t}, [r{s}+0x{m:X}]"),
            "100110_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("lwr r{t}, [r{s}+0x{m:X}]"),
            "101000_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("sb r{t}, [r{s}+0x{m:X}]"),
            "101001_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("sh r{t}, [r{s}+0x{m:X}]"),
            "101010_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("swl r{t}, [r{s}+0x{m:X}]"),
            "101011_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("sw r{t}, [r{s}+0x{m:X}]"),
            "101110_sssss_ttttt_mmmmm_mmmmm_mmmmmm" => format!("swr r{t}, [r{s}+0x{m:X}]"),

            _ => format!("{:08X}??", inst),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Inst(pub(crate) u32);

impl Inst {
    pub fn rs(self) -> u32 {
        self.0.bits(21, 5)
    }

    pub fn rt(self) -> u32 {
        self.0.bits(16, 5)
    }

    pub fn rd(self) -> u32 {
        self.0.bits(11, 5)
    }

    pub fn imm5(self) -> u32 {
        self.0.bits(6, 5)
    }

    pub fn imm16(self) -> u32 {
        self.0.low().u32()
    }

    pub fn imm16s(self) -> i32 {
        self.0.low() as i16 as i32
    }

    pub fn imm26(self) -> u32 {
        self.0.bits(0, 26)
    }
}
