// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

//! Handlers for THUMB instructions.

use bitmatch::bitmatch;
use common::numutil::{NumExt, U16Ext};

use super::interface::{ArmSystem, SysWrapper};
use crate::{access::*, registers::Flag::*, Cpu};

pub type ThumbHandler<S> = fn(&mut SysWrapper<S>, ThumbInst);
pub type ThumbLut<S> = [ThumbHandler<S>; 256];

impl<S: ArmSystem> SysWrapper<S> {
    pub fn execute_inst_thumb(&mut self, inst: u16) {
        let handler = Self::get_handler_thumb(inst);
        handler(self, ThumbInst(inst));
    }

    pub fn get_handler_thumb(inst: u16) -> ThumbHandler<S> {
        S::THUMB_LUT[inst.us() >> 8]
    }

    pub fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        self.und_inst(inst.0);
    }

    // THUMB.1/2
    pub fn thumb_arithmetic<const KIND: &'static str>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let s = inst.low(3);
        let n = inst.0.bits(6, 5);
        let rs = self.low(s);
        let value = match KIND {
            "LSL" => self.cpu().lsl::<true>(rs, n.u32()),
            "LSR" => self.cpu().lsr::<true, true>(rs, n.u32()),
            "ASR" => self.cpu().asr::<true, true>(rs, n.u32()),
            "ADDR" => {
                let rn = self.low(n & 7);
                self.cpu().add::<true>(rs, rn)
            }
            "ADDI" => self.cpu().add::<true>(rs, (n & 7).u32()),
            "SUBR" => {
                let rn = self.low(n & 7);
                self.cpu().sub::<true>(rs, rn)
            }
            "SUBI" => self.cpu().sub::<true>(rs, (n & 7).u32()),
            _ => panic!("Invalid arithmetic"),
        };
        self.cpu().registers[d.us()] = value;
    }

    // THUMB.3
    pub fn thumb_3<const KIND: &'static str>(&mut self, inst: ThumbInst) {
        let d = inst.low(8);
        let n = inst.0 & 0xFF;
        let rd = self.low(d);
        match KIND {
            "MOV" => {
                self.cpu().set_nz::<true>(n.u32());
                self.cpu().registers[d.us()] = n.u32();
            }
            "CMP" => {
                self.cpu().sub::<true>(rd, n.u32());
            }
            "ADD" => self.cpu().registers[d.us()] = self.cpu().add::<true>(rd, n.u32()),
            "SUB" => self.cpu().registers[d.us()] = self.cpu().sub::<true>(rd, n.u32()),
            _ => panic!("Invalid arithmetic"),
        };
    }

    // THUMB.4
    pub fn thumb_alu(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let s = inst.low(3);
        let o = inst.0.bits(6, 4);

        let rd = self.low(d);
        let rs = self.low(s);

        self.cpu().registers[d.us()] = match o {
            0x0 => self.cpu().and::<true>(rd, rs),
            0x1 => self.cpu().xor::<true>(rd, rs),
            0x2 => {
                self.idle_nonseq();
                self.cpu().lsl::<true>(rd, rs & 0xFF)
            }
            0x3 => {
                self.idle_nonseq();
                self.cpu().lsr::<true, false>(rd, rs & 0xFF)
            }
            0x4 => {
                self.idle_nonseq();
                self.cpu().asr::<true, false>(rd, rs & 0xFF)
            }
            0x5 => {
                let c = self.cpur().flag(Carry) as u32;
                self.cpu().adc::<true>(rd, rs, c)
            }
            0x6 => {
                let c = self.cpur().flag(Carry) as u32;
                self.cpu().sbc::<true>(rd, rs, c)
            }
            0x7 => {
                self.idle_nonseq();
                self.cpu().ror::<true, false>(rd, rs & 0xFF)
            }
            0x8 => {
                // TST
                self.cpu().and::<true>(rd, rs);
                rd
            }
            0x9 => self.cpu().neg::<true>(rs),
            0xA => {
                // CMP
                self.cpu().sub::<true>(rd, rs);
                rd
            }
            0xB => {
                // CMN
                self.cpu().add::<true>(rd, rs);
                rd
            }
            0xC => self.cpu().or::<true>(rd, rs),
            0xD => {
                self.mul_wait_cycles(rd, true);
                self.cpu().mul::<true>(rd, rs)
            }
            0xE => self.cpu().bit_clear::<true>(rd, rs),
            _ => self.cpu().not::<true>(rs),
        }
    }

    // THUMB.5
    pub fn thumb_hi_add(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        let res = self.reg(d.u32()).wrapping_add(self.reg(s.u32()));
        self.set_reg(d.u32(), res);
    }

    pub fn thumb_hi_cmp(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        let rs = self.reg(s.u32());
        let rd = self.reg(d.u32());
        self.cpu().sub::<true>(rd, rs);
    }

    pub fn thumb_hi_mov(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        self.set_reg(d.u32(), self.reg(s.u32()));
    }

    pub fn thumb_hi_bx(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        if inst.0.is_bit(7) {
            // BLX
            let rn = self.reg(s.u32());
            // Is this v5 behavior correct?
            if S::IS_V5 {
                let pc = self.cpur().pc() - 1;
                self.cpu().set_lr(pc);
                if !rn.is_bit(0) {
                    self.cpu().set_flag(Thumb, false);
                }
            }
            self.set_pc(rn);
        } else if s == 15 {
            // BX ARM switch
            self.cpu().set_flag(Thumb, false);
            self.set_pc(self.cpur().pc()); // Align
        } else {
            // BX
            if self.reg(s.u32()).is_bit(0) {
                self.set_pc(self.reg(s.u32()) & !1);
            } else {
                self.cpu().set_flag(Thumb, false);
                self.set_pc(self.reg(s.u32()) & !3);
            }
        }
    }

    // THUMB.6
    pub fn thumb_ldr6(&mut self, inst: ThumbInst) {
        let d = inst.low(8);
        let n = inst.0 & 0xFF;

        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().adj_pc() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.7/8
    pub fn thumb_ldrstr78<const O: u16>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let rb = self.cpu().low(inst.low(6));
        let ro = self.cpu().low(inst.low(3));
        let rd = self.cpu().low(d);
        let addr = rb.wrapping_add(ro);
        self.cpu().access_type = NONSEQ;

        match O {
            0 => self.write::<u32>(addr, rd, NONSEQ),       // STR
            1 => self.write::<u16>(addr, rd.u16(), NONSEQ), // STRH
            2 => self.write::<u8>(addr, rd.u8(), NONSEQ),   // STRB
            3 => self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32, /* LDSB */
            4 => self.cpu().registers[d.us()] = self.read_word_ldrswp(addr, NONSEQ), // LDR
            5 => self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ),      // LDRH
            6 => self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ).u32(), // LDRB
            // LDSH, needs special handling for unaligned reads which makes it behave as
            // LBSB
            _ if addr.is_bit(0) => {
                self.cpu().registers[d.us()] = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32;
            }
            _ => self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ) as i16 as i32 as u32,
        }
        if O > 2 {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.9
    pub fn thumb_ldrstr9<const O: u16>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let rb = self.cpu().low(inst.low(3));
        let rd = self.cpu().low(d);
        let n = inst.0.bits(6, 5);
        self.cpu().access_type = NONSEQ;

        match O {
            0 => self.write::<u32>(rb + (n.u32() << 2), rd, NONSEQ), // STR
            1 => self.cpu().registers[d.us()] = self.read_word_ldrswp(rb + (n.u32() << 2), NONSEQ), /* LDR */
            2 => self.write::<u8>(rb + n.u32(), rd.u8(), NONSEQ), // STRB
            _ => self.cpu().registers[d.us()] = self.read::<u8>(rb + n.u32(), NONSEQ).u32(), // LDRB
        }

        if O.is_bit(0) {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.10
    pub fn thumb_ldrstr10<const STR: bool>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let n = inst.0.bits(6, 5);
        let rb = self.cpu().low(inst.low(3));
        let ro = n.u32() << 1; // Step 2
        let rd = self.cpu().low(d);
        let addr = rb + ro;
        self.cpu().access_type = NONSEQ;

        if STR {
            self.write::<u16>(addr, rd.u16(), NONSEQ);
        } else {
            self.cpu().registers[d.us()] = self.read::<u16>(addr, NONSEQ).u32();
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.11
    pub fn thumb_str_sp(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        let rd = self.low(d);
        let addr = self.cpur().sp() + (n.u32() << 2);
        self.cpu().access_type = NONSEQ;
        self.write::<u32>(addr, rd, NONSEQ);
    }

    pub fn thumb_ldr_sp(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        self.cpu().registers[d.us()] =
            self.read_word_ldrswp(self.cpur().sp() + (n.u32() << 2), NONSEQ);
        // LDR has +1I
        self.idle_nonseq();
    }

    // THUMB.12
    pub fn thumb_rel_addr<const SP: bool>(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        if SP {
            self.cpu().registers[d.us()] = self.cpu().sp() + (n.u32() << 2);
        } else {
            self.cpu().registers[d.us()] = self.cpu().adj_pc() + (n.u32() << 2);
        }
    }

    // THUMB.13
    pub fn thumb_sp_offs(&mut self, inst: ThumbInst) {
        let n = (inst.0 & 0x7F) << 2;
        let sp = self.cpur().sp();
        if inst.0.is_bit(7) {
            self.cpu().set_sp(sp - (n as u32));
        } else {
            self.cpu().set_sp(sp + (n as u32));
        }
    }

    // THUMB.14
    pub fn thumb_push<const SP: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;
        // PUSH
        if SP {
            sp -= 4;
            let lr = self.cpur().lr();
            self.write::<u32>(sp, lr, kind);
            kind = SEQ;
        }

        for reg in (0..8).rev() {
            if inst.0.is_bit(reg) {
                sp -= 4;
                let reg = self.cpur().registers[reg.us()];
                self.write::<u32>(sp, reg, kind);
                kind = SEQ;
            }
        }
        assert!(kind == SEQ);
        self.cpu().set_sp(sp);
        self.cpu().access_type = NONSEQ;
    }

    pub fn thumb_pop<const PC: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu().sp();
        let mut kind = NONSEQ;
        // POP
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                self.cpu().registers[reg.us()] = self.read::<u32>(sp, kind);
                sp += 4;
                kind = SEQ;
            }
        }
        if PC {
            let pc = self.read::<u32>(sp, kind);
            if S::IS_V5 && !pc.is_bit(0) {
                self.cpu().set_flag(Thumb, false);
            }
            self.set_pc(pc);
            sp += 4;
            kind = SEQ;
        }
        assert!(kind == SEQ);
        self.cpu().set_sp(sp);
        self.idle_nonseq();
    }

    // THUMB.15
    pub fn thumb_stmia(&mut self, inst: ThumbInst) {
        let b = inst.low(8);
        let mut kind = NONSEQ;
        let mut base_rlist_addr = None;
        let mut rb = self.low(b);
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                if reg == b && kind != NONSEQ {
                    base_rlist_addr = Some(self.low(b));
                }
                let reg = self.low(reg);
                self.write::<u32>(rb, reg, kind);
                rb = rb.wrapping_add(4);
                self.cpu().registers[b.us()] = rb;
                kind = SEQ;
            }
        }
        if let Some(addr) = base_rlist_addr {
            // If base was in Rlist and not the first, write final address to that location.
            // We ignore timing since this was already (wrongly) written in the loop above.
            self.set::<u32>(addr, rb);
        }
        if kind == NONSEQ {
            self.on_empty_rlist(b.u32(), true, true, false);
        }
        self.cpu().access_type = NONSEQ;
    }

    pub fn thumb_ldmia(&mut self, inst: ThumbInst) {
        let b = inst.low(8);
        let mut kind = NONSEQ;
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                let addr = self.low(b);
                self.cpu().registers[reg.us()] = self.read::<u32>(addr, kind);
                self.cpu().registers[b.us()] = self.low(b).wrapping_add(4);
                kind = SEQ;
            }
        }
        if kind == NONSEQ {
            self.on_empty_rlist(b.u32(), false, true, false);
        }
        self.idle_nonseq();
    }

    // THUMB.16
    pub fn thumb_bcond<const COND: u16>(&mut self, inst: ThumbInst) {
        let condition = self.cpu().eval_condition(COND);
        if condition {
            let nn = ((inst.0 & 0xFF) as i8 as i32) * 2; // Step 2
            let pc = self.cpur().pc();
            self.cpu().is_halted = !self.cpu().waitloop.on_jump(pc, nn);
            self.set_pc(pc.wrapping_add_signed(nn));
        }
    }

    // THUMB.17
    pub fn thumb_swi(&mut self, _inst: ThumbInst) {
        self.swi();
    }

    // THUMB.18
    pub fn thumb_br(&mut self, inst: ThumbInst) {
        let nn = (inst.0.i10() as i32) * 2; // Step 2
        let pc = self.cpur().pc();
        self.cpu().is_halted = !self.cpu().waitloop.on_jump(pc, nn);
        self.set_pc(pc.wrapping_add_signed(nn));
    }

    // THUMB.19
    pub fn thumb_set_lr(&mut self, inst: ThumbInst) {
        let lr = self
            .cpur()
            .pc()
            .wrapping_add_signed((inst.0.i10() as i32) << 12);
        self.cpu().set_lr(lr);
    }

    pub fn thumb_bl<const THUMB: bool>(&mut self, inst: ThumbInst) {
        let pc = self.cpu().pc();
        self.set_pc(self.cpur().lr().wrapping_add(inst.0.bits(0, 11).u32() << 1));
        self.cpu().set_lr(pc - 1);
        self.cpu().set_flag(Thumb, THUMB);
    }
}

impl<S: ArmSystem> Cpu<S> {
    #[bitmatch]
    pub fn get_mnemonic_thumb(inst: u16) -> String {
        #[bitmatch]
        match inst {
            "11011111_nnnnnnnn" => format!("swi 0x{:02X}", n),

            "000_00nnnnnsssddd" => format!("lsl r{d}, r{s}, #0x{:X}", n),
            "000_01nnnnnsssddd" => format!("lsr r{d}, r{s}, #0x{:X}", n),
            "000_10nnnnnsssddd" => format!("asr r{d}, r{s}, #0x{:X}", n),
            "00011_00nnnsssddd" => format!("add r{d}, r{s}, r{n}"),
            "00011_01nnnsssddd" => format!("sub r{d}, r{s}, r{n}"),
            "00011_10nnnsssddd" => format!("add r{d}, r{s}, #0x{:X}", n),
            "00011_11nnnsssddd" => format!("sub r{d}, r{s}, #0x{:X}", n),

            "001_00dddnnnnnnnn" => format!("mov r{d}, #{n}"),
            "001_01dddnnnnnnnn" => format!("cmp r{d}, #{n}"),
            "001_10dddnnnnnnnn" => format!("add r{d}, #{n}"),
            "001_11dddnnnnnnnn" => format!("sub r{d}, #{n}"),

            "010000_oooosssddd" => {
                let op = match o {
                    0x0 => "and",
                    0x1 => "eor",
                    0x2 => "lsl",
                    0x3 => "lsr",
                    0x4 => "asr",
                    0x5 => "add",
                    0x6 => "sub",
                    0x7 => "ror",
                    0x8 => "tst",
                    0x9 => "neg",
                    0xA => "cmp",
                    0xB => "cmn",
                    0xC => "orr",
                    0xD => "mul",
                    0xE => "bic",
                    _ => "mvn",
                };
                if o == 0x8 {
                    format!("{op} r{s}")
                } else {
                    format!("{op} r{d}, r{s}")
                }
            }

            "010001_00dssssddd" => format!("add r{d}, r{s}"),
            "010001_01dssssddd" => format!("cmp r{d}, r{s}"),
            "010001_10dssssddd" => format!("mov r{d}, r{s}"),
            "010001_110ssss???" => format!("bx r{s}"),
            "010001_111ssss???" => format!("blx r{s}"),
            "01001_dddnnnnnnnn" => format!("ldr r{d}, [PC, #0x{:X}]", (n.u32() << 2)),
            "0101_ooosssbbbddd" => {
                let op = match o {
                    0 => "str",
                    1 => "strh",
                    2 => "strb",
                    3 => "ldsb",
                    4 => "ldr",
                    5 => "ldrh",
                    6 => "ldrb",
                    _ => "ldsh",
                };
                format!("{op} r{d}, [r{b}, r{s}]")
            }
            "011_oonnnnnbbbddd" => {
                let op = match o {
                    0 => "str",
                    1 => "ldr",
                    2 => "strb",
                    _ => "ldrb",
                };
                format!("{op} r{d}, [r{b}, #0x{:X}]", n)
            }
            "1000_0nnnnnbbbddd" => format!("strh r{d}, [r{b}, #0x{:X}]", n << 1),
            "1000_1nnnnnbbbddd" => format!("ldrh r{d}, [r{b}, #0x{:X}]", n << 1),
            "1001_0dddnnnnnnnn" => format!("str r{d}, [sp, #0x{:X}]", n << 2),
            "1001_1dddnnnnnnnn" => format!("ldr r{d}, [sp, #0x{:X}]", n << 2),

            "1010_0dddnnnnnnnn" => format!("add r{d}, pc, #0x{:X}", n << 2),
            "1010_1dddnnnnnnnn" => format!("add r{d}, sp, #0x{:X}", n << 2),

            "10110000_0nnnnnnn" => format!("add sp, #0x{:X}", n << 2),
            "10110000_1nnnnnnn" => format!("add sp, #-0x{:X}", n << 2),

            "1011_0100rrrrrrrr" => format!("push {:08b}", r),
            "1011_0101rrrrrrrr" => format!("push {:08b}, lr", r),
            "1011_1100rrrrrrrr" => format!("pop {:08b}", r),
            "1011_1101rrrrrrrr" => format!("pop {:08b}, pc", r),
            "1100_0bbbrrrrrrrr" => format!("stmia r{b}!, {:08b}", r),
            "1100_1bbbrrrrrrrr" => format!("ldmia r{b}!, {:08b}", r),

            "1101_ccccnnnnnnnn" => format!(
                "b{} 0x{:X}",
                Cpu::<S>::condition_mnemonic(c).to_ascii_lowercase(),
                ((n as i8 as i16) * 2) + 2
            ),
            "11100_nnnnnnnnnnn" => format!("b 0x{:X}", (n.i10() << 1) + 2),
            "11110_nnnnnnnnnnn" => format!("mov lr, (pc + 0x{:X})", n << 12),
            "11111_nnnnnnnnnnn" => format!("bl lr + 0x{:X}", n << 1),
            "11101_nnnnnnnnnnn" => format!("blx lr + 0x{:X}", n << 1),

            _ => format!("{:04X}??", inst),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ThumbInst(pub u16);

impl ThumbInst {
    pub fn low(self, idx: u16) -> u16 {
        self.0.bits(idx, 3)
    }
}
