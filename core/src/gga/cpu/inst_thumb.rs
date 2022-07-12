// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use bitmatch::bitmatch;

use crate::{
    gga::{
        cpu::{registers::Flag::*, Cpu, Exception},
        Access::*,
        GameGirlAdv,
    },
    numutil::{NumExt, U16Ext},
};

type ThumbHandler = fn(&mut GameGirlAdv, ThumbInst);
type ThumbLut = [ThumbHandler; 256];
const THUMB_LUT: ThumbLut = GameGirlAdv::make_thumb_lut();

impl GameGirlAdv {
    const fn make_thumb_lut() -> ThumbLut {
        let mut lut: ThumbLut = [Self::thumb_unknown_opcode; 256];

        lut[0b11011111] = Self::thumb_swi;
        lut[0b10110000] = Self::thumb_sp_offs;

        lut[0b01000100] = Self::thumb_hi_add;
        lut[0b01000101] = Self::thumb_hi_cmp;
        lut[0b01000110] = Self::thumb_hi_mov;
        lut[0b01000111] = Self::thumb_hi_bx;

        Self::lut_span(&mut lut, 0b00000, 5, Self::thumb_arithmetic::<"LSL">);
        Self::lut_span(&mut lut, 0b00001, 5, Self::thumb_arithmetic::<"LSR">);
        Self::lut_span(&mut lut, 0b00010, 5, Self::thumb_arithmetic::<"ASR">);
        Self::lut_span(&mut lut, 0b0001100, 7, Self::thumb_arithmetic::<"ADDR">);
        Self::lut_span(&mut lut, 0b0001101, 7, Self::thumb_arithmetic::<"SUBR">);
        Self::lut_span(&mut lut, 0b0001110, 7, Self::thumb_arithmetic::<"ADDI">);
        Self::lut_span(&mut lut, 0b0001111, 7, Self::thumb_arithmetic::<"SUBI">);

        Self::lut_span(&mut lut, 0b00100, 5, Self::thumb_3::<"MOV">);
        Self::lut_span(&mut lut, 0b00101, 5, Self::thumb_3::<"CMP">);
        Self::lut_span(&mut lut, 0b00110, 5, Self::thumb_3::<"ADD">);
        Self::lut_span(&mut lut, 0b00111, 5, Self::thumb_3::<"SUB">);

        Self::lut_span(&mut lut, 0b010000, 6, Self::thumb_alu);
        Self::lut_span(&mut lut, 0b01001, 5, Self::thumb_ldr6);

        Self::lut_span(&mut lut, 0b0101000, 7, Self::thumb_ldrstr78::<0>);
        Self::lut_span(&mut lut, 0b0101001, 7, Self::thumb_ldrstr78::<1>);
        Self::lut_span(&mut lut, 0b0101010, 7, Self::thumb_ldrstr78::<2>);
        Self::lut_span(&mut lut, 0b0101011, 7, Self::thumb_ldrstr78::<3>);
        Self::lut_span(&mut lut, 0b0101100, 7, Self::thumb_ldrstr78::<4>);
        Self::lut_span(&mut lut, 0b0101101, 7, Self::thumb_ldrstr78::<5>);
        Self::lut_span(&mut lut, 0b0101110, 7, Self::thumb_ldrstr78::<6>);
        Self::lut_span(&mut lut, 0b0101111, 7, Self::thumb_ldrstr78::<7>);

        Self::lut_span(&mut lut, 0b01100, 5, Self::thumb_ldrstr9::<0>);
        Self::lut_span(&mut lut, 0b01101, 5, Self::thumb_ldrstr9::<1>);
        Self::lut_span(&mut lut, 0b01110, 5, Self::thumb_ldrstr9::<2>);
        Self::lut_span(&mut lut, 0b01111, 5, Self::thumb_ldrstr9::<3>);
        Self::lut_span(&mut lut, 0b10000, 5, Self::thumb_ldrstr10::<true>);
        Self::lut_span(&mut lut, 0b10001, 5, Self::thumb_ldrstr10::<false>);
        Self::lut_span(&mut lut, 0b10010, 5, Self::thumb_str_sp);
        Self::lut_span(&mut lut, 0b10011, 5, Self::thumb_ldr_sp);

        Self::lut_span(&mut lut, 0b10100, 5, Self::thumb_rel_addr::<false>);
        Self::lut_span(&mut lut, 0b10101, 5, Self::thumb_rel_addr::<true>);

        lut[0b10110100] = Self::thumb_push::<false>;
        lut[0b10110101] = Self::thumb_push::<true>;
        lut[0b10111100] = Self::thumb_pop::<false>;
        lut[0b10111101] = Self::thumb_pop::<true>;
        Self::lut_span(&mut lut, 0b11000, 5, Self::thumb_stmia);
        Self::lut_span(&mut lut, 0b11001, 5, Self::thumb_ldmia);

        // Ugh.
        lut[0xD0] = Self::thumb_bcond::<0x0>;
        lut[0xD1] = Self::thumb_bcond::<0x1>;
        lut[0xD2] = Self::thumb_bcond::<0x2>;
        lut[0xD3] = Self::thumb_bcond::<0x3>;
        lut[0xD4] = Self::thumb_bcond::<0x4>;
        lut[0xD5] = Self::thumb_bcond::<0x5>;
        lut[0xD6] = Self::thumb_bcond::<0x6>;
        lut[0xD7] = Self::thumb_bcond::<0x7>;
        lut[0xD8] = Self::thumb_bcond::<0x8>;
        lut[0xD9] = Self::thumb_bcond::<0x9>;
        lut[0xDA] = Self::thumb_bcond::<0xA>;
        lut[0xDB] = Self::thumb_bcond::<0xB>;
        lut[0xDC] = Self::thumb_bcond::<0xC>;
        lut[0xDD] = Self::thumb_bcond::<0xD>;
        lut[0xDE] = Self::thumb_bcond::<0xE>;

        Self::lut_span(&mut lut, 0b11100, 5, Self::thumb_br);
        Self::lut_span(&mut lut, 0b11110, 5, Self::thumb_set_lr);
        Self::lut_span(&mut lut, 0b11101, 5, Self::thumb_bl::<false>);
        Self::lut_span(&mut lut, 0b11111, 5, Self::thumb_bl::<true>);

        lut
    }

    pub fn execute_inst_thumb(&mut self, inst: u16) {
        let handler = THUMB_LUT[inst.us() >> 8];
        handler(self, ThumbInst(inst))
    }

    fn thumb_unknown_opcode(&mut self, inst: ThumbInst) {
        Self::log_unknown_opcode(inst.0);
    }

    // THUMB.1/2
    fn thumb_arithmetic<const KIND: &'static str>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let s = inst.low(3);
        let n = inst.0.bits(6, 5);
        let value = match KIND {
            "LSL" => self.cpu.lsl(self.low(s), n.u32()),
            "LSR" => self.cpu.lsr::<true>(self.low(s), n.u32()),
            "ASR" => self.cpu.asr::<true>(self.low(s), n.u32()),
            "ADDR" => self.cpu.add(self.low(s), self.low(n & 7)),
            "ADDI" => self.cpu.add(self.low(s), (n & 7).u32()),
            "SUBR" => self.cpu.sub(self.low(s), self.low(n & 7)),
            "SUBI" => self.cpu.sub(self.low(s), (n & 7).u32()),
            _ => panic!("Invalid arithmetic"),
        };
        self.cpu.low[d.us()] = value;
    }

    // THUMB.3
    fn thumb_3<const KIND: &'static str>(&mut self, inst: ThumbInst) {
        let d = inst.low(8);
        let n = inst.0 & 0xFF;
        match KIND {
            "MOV" => {
                self.cpu.set_zn(n.u32());
                self.cpu.low[d.us()] = n.u32();
            }
            "CMP" => {
                let rd = self.low(d);
                self.cpu.sub(rd, n.u32());
            }
            "ADD" => self.cpu.low[d.us()] = self.cpu.add(self.low(d), n.u32()),
            "SUB" => self.cpu.low[d.us()] = self.cpu.sub(self.low(d), n.u32()),
            _ => panic!("Invalid arithmetic"),
        };
    }

    // THUMB.4
    fn thumb_alu(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let s = inst.low(3);
        let o = inst.0.bits(6, 4);

        let rd = self.low(d);
        let rs = self.low(s);

        self.cpu.low[d.us()] = match o {
            0x0 => self.cpu.and(rd, rs),
            0x1 => self.cpu.xor(rd, rs),
            0x2 => {
                self.idle_nonseq();
                self.cpu.lsl(rd, rs & 0xFF)
            }
            0x3 => {
                self.idle_nonseq();
                self.cpu.lsr::<false>(rd, rs & 0xFF)
            }
            0x4 => {
                self.idle_nonseq();
                self.cpu.asr::<false>(rd, rs & 0xFF)
            }
            0x5 => self.cpu.adc(rd, rs, self.cpu.flag(Carry) as u32),
            0x6 => self.cpu.sbc(rd, rs, self.cpu.flag(Carry) as u32),
            0x7 => {
                self.idle_nonseq();
                self.cpu.ror::<false>(rd, rs & 0xFF)
            }
            0x8 => {
                // TST
                self.cpu.and(rd, rs);
                rd
            }
            0x9 => self.cpu.neg(rs),
            0xA => {
                // CMP
                self.cpu.sub(rd, rs);
                rd
            }
            0xB => {
                // CMN
                self.cpu.add(rd, rs);
                rd
            }
            0xC => self.cpu.or(rd, rs),
            0xD => {
                self.mul_wait_cycles(rd, true);
                self.cpu.mul(rd, rs)
            }
            0xE => self.cpu.bit_clear(rd, rs),
            _ => self.cpu.not(rs),
        }
    }

    // THUMB.5
    fn thumb_hi_add(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        let res = self.reg(d.u32()).wrapping_add(self.reg(s.u32()));
        self.set_reg(d.u32(), res);
    }

    fn thumb_hi_cmp(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        self.cpu.sub(self.reg(d.u32()), self.reg(s.u32()));
    }

    fn thumb_hi_mov(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        let d = inst.low(0) | (inst.0.bit(7) << 3);
        self.set_reg(d.u32(), self.reg(s.u32()));
    }

    fn thumb_hi_bx(&mut self, inst: ThumbInst) {
        let s = inst.0.bits(3, 4);
        if inst.0.is_bit(7) {
            // BLX
            self.set_pc(self.reg(s.u32()));
        } else if s == 15 {
            // BX ARM switch
            self.cpu.set_flag(Thumb, false);
            self.set_pc(self.cpu.pc); // Align
        } else {
            // BX
            if !self.reg(s.u32()).is_bit(0) {
                self.cpu.set_flag(Thumb, false);
                self.set_pc(self.reg(s.u32()) & !3);
            } else {
                self.set_pc(self.reg(s.u32()) & !1);
            }
        }
    }

    // THUMB.6
    fn thumb_ldr6(&mut self, inst: ThumbInst) {
        let d = inst.low(8);
        let n = inst.0 & 0xFF;

        // LDR has +1I
        self.idle_nonseq();
        self.cpu.low[d.us()] = self.read_word_ldrswp(self.cpu.adj_pc() + (n.u32() << 2), NonSeq)
    }

    // THUMB.7/8
    fn thumb_ldrstr78<const O: u16>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let rb = self.cpu.low(inst.low(6));
        let ro = self.cpu.low(inst.low(3));
        let rd = self.cpu.low(d);
        let addr = rb.wrapping_add(ro);
        self.cpu.access_type = NonSeq;

        match O {
            0 => self.write_word(addr, rd, NonSeq),        // STR
            1 => self.write_hword(addr, rd.u16(), NonSeq), // STRH
            2 => self.write_byte(addr, rd.u8(), NonSeq),   // STRB
            3 => self.cpu.low[d.us()] = self.read_byte(addr, NonSeq) as i8 as i32 as u32, // LDSB
            4 => self.cpu.low[d.us()] = self.read_word_ldrswp(addr, NonSeq), // LDR
            5 => self.cpu.low[d.us()] = self.read_hword(addr, NonSeq), // LDRH
            6 => self.cpu.low[d.us()] = self.read_byte(addr, NonSeq).u32(), // LDRB
            // LDSH, needs special handling for unaligned reads which makes it behave as
            // LBSB
            _ if addr.is_bit(0) => {
                self.cpu.low[d.us()] = self.read_byte(addr, NonSeq) as i8 as i32 as u32
            }
            _ => self.cpu.low[d.us()] = self.read_hword(addr, NonSeq) as i16 as i32 as u32,
        }
        if O > 2 {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.9
    fn thumb_ldrstr9<const O: u16>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let rb = self.cpu.low(inst.low(3));
        let rd = self.cpu.low(d);
        let n = inst.0.bits(6, 5);
        self.cpu.access_type = NonSeq;

        match O {
            0 => self.write_word(rb + (n.u32() << 2), rd, NonSeq), // STR
            1 => self.cpu.low[d.us()] = self.read_word_ldrswp(rb + (n.u32() << 2), NonSeq), // LDR
            2 => self.write_byte(rb + n.u32(), rd.u8(), NonSeq),   // STRB
            _ => self.cpu.low[d.us()] = self.read_byte(rb + n.u32(), NonSeq).u32(), // LDRB
        }

        if O.is_bit(0) {
            // LDR has +1I
            self.add_i_cycles(1);
        }
    }

    // THUMB.10
    fn thumb_ldrstr10<const STR: bool>(&mut self, inst: ThumbInst) {
        let d = inst.low(0);
        let n = inst.0.bits(6, 5);
        let rb = self.cpu.low(inst.low(3));
        let ro = n.u32() << 1; // Step 2
        let rd = self.cpu.low(d);
        let addr = rb + ro;
        self.cpu.access_type = NonSeq;

        if STR {
            self.write_hword(addr, rd.u16(), NonSeq);
        } else {
            // LDR has +1I
            self.add_i_cycles(1);
            self.cpu.low[d.us()] = self.read_hword(addr, NonSeq).u32();
        }
    }

    // THUMB.11
    fn thumb_str_sp(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        self.cpu.access_type = NonSeq;
        self.write_word(self.cpu.sp() + (n.u32() << 2), self.cpu.low(d), NonSeq)
    }

    fn thumb_ldr_sp(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        // LDR has +1I
        self.idle_nonseq();
        self.cpu.low[d.us()] = self.read_word_ldrswp(self.cpu.sp() + (n.u32() << 2), NonSeq);
    }

    // THUMB.12
    fn thumb_rel_addr<const SP: bool>(&mut self, inst: ThumbInst) {
        let n = inst.0 & 0xFF;
        let d = inst.low(8);
        if SP {
            self.cpu.low[d.us()] = self.cpu.sp() + (n.u32() << 2);
        } else {
            self.cpu.low[d.us()] = self.cpu.adj_pc() + (n.u32() << 2);
        }
    }

    // THUMB.13
    fn thumb_sp_offs(&mut self, inst: ThumbInst) {
        let n = (inst.0 & 0x7F) << 2;
        if inst.0.is_bit(7) {
            self.cpu.set_sp(self.cpu.sp() - (n as u32));
        } else {
            self.cpu.set_sp(self.cpu.sp() + (n as u32));
        }
    }

    // THUMB.14
    fn thumb_push<const SP: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu.sp();
        let mut kind = NonSeq;
        // PUSH
        if SP {
            sp -= 4;
            self.write_word(sp, self.cpu.lr(), kind);
            kind = Seq;
        }

        for reg in (0..8).rev() {
            if inst.0.is_bit(reg) {
                sp -= 4;
                self.write_word(sp, self.cpu.low[reg.us()], kind);
                kind = Seq;
            }
        }
        assert!(kind == Seq);
        self.cpu.set_sp(sp);
        self.cpu.access_type = NonSeq;
    }

    fn thumb_pop<const PC: bool>(&mut self, inst: ThumbInst) {
        let mut sp = self.cpu.sp();
        let mut kind = NonSeq;
        // POP
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                self.cpu.low[reg.us()] = self.read_word(sp, kind);
                sp += 4;
                kind = Seq;
            }
        }
        if PC {
            let pc = self.read_word(sp, kind);
            self.set_pc(pc);
            sp += 4;
            kind = Seq;
        }
        assert!(kind == Seq);
        self.cpu.set_sp(sp);
        self.idle_nonseq();
    }

    // THUMB.15
    fn thumb_stmia(&mut self, inst: ThumbInst) {
        let b = inst.low(8);
        let mut kind = NonSeq;
        let mut base_rlist_addr = None;
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                if reg == b && kind != NonSeq {
                    base_rlist_addr = Some(self.cpu.low(b))
                }
                self.write_word(self.cpu.low(b), self.cpu.low[reg.us()], kind);
                self.cpu.low[b.us()] = self.low(b).wrapping_add(4);
                kind = Seq;
            }
        }
        if let Some(addr) = base_rlist_addr {
            // If base was in Rlist and not the first, write final address to that location.
            // We ignore timing since this was already (wrongly) written in the loop above.
            self.set_word(addr, self.cpu.low[b.us()]);
        }
        if kind == NonSeq {
            self.on_empty_rlist(b.u32(), true, true, false);
        }
        self.cpu.access_type = NonSeq;
    }

    fn thumb_ldmia(&mut self, inst: ThumbInst) {
        let b = inst.low(8);
        let mut kind = NonSeq;
        for reg in 0..8 {
            if inst.0.is_bit(reg) {
                self.cpu.low[reg.us()] = self.read_word(self.cpu.low(b), kind);
                self.cpu.low[b.us()] = self.low(b).wrapping_add(4);
                kind = Seq;
            }
        }
        if kind == NonSeq {
            self.on_empty_rlist(b.u32(), false, true, false);
        }
        self.idle_nonseq();
    }

    // THUMB.16
    fn thumb_bcond<const COND: u16>(&mut self, inst: ThumbInst) {
        if inst.0.bits(8, 4) != COND {
            println!("{COND:X}{:X}", inst.0);
        }
        let condition = self.cpu.eval_condition(COND);
        if condition {
            let nn = ((inst.0 & 0xFF) as i8 as i32) * 2; // Step 2
            self.set_pc(self.cpu.pc.wrapping_add_signed(nn));
        }
    }

    // THUMB.17
    fn thumb_swi(&mut self, _inst: ThumbInst) {
        self.swi();
    }

    // THUMB.18
    fn thumb_br(&mut self, inst: ThumbInst) {
        let nn = (inst.0.i10() as i32) * 2; // Step 2
        self.set_pc(self.cpu.pc.wrapping_add_signed(nn));
    }

    // THUMB.19
    fn thumb_set_lr(&mut self, inst: ThumbInst) {
        self.cpu
            .set_lr(self.cpu.pc.wrapping_add_signed((inst.0.i10() as i32) << 12));
    }

    fn thumb_bl<const THUMB: bool>(&mut self, inst: ThumbInst) {
        let pc = self.cpu.pc;
        self.set_pc(self.cpu.lr().wrapping_add(inst.0.bits(0, 11).u32() << 1));
        self.cpu.set_lr(pc - 1);
        self.cpu.set_flag(Thumb, THUMB);
    }

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
                Cpu::condition_mnemonic(c).to_ascii_lowercase(),
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
struct ThumbInst(u16);

impl ThumbInst {
    fn low(&self, idx: u16) -> u16 {
        self.0.bits(idx, 3)
    }
}
