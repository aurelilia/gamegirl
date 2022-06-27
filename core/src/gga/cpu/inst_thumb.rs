use bitmatch::bitmatch;

use crate::{
    gga::{
        cpu::{registers::Flag::*, Cpu, Exception},
        Access::*,
        GameGirlAdv,
    },
    numutil::{NumExt, U16Ext},
};

impl GameGirlAdv {
    #[bitmatch]
    pub fn execute_inst_thumb(&mut self, inst: u16) {
        #[bitmatch]
        match inst {
            // SWI
            "11011111_????????" => {
                Cpu::exception_occurred(self, Exception::Swi);
                self.memory.bios_value = 0xE3A02004;
            }

            // THUMB.1
            "000_00nnnnnsssddd" => self.cpu.low[d.us()] = self.cpu.lsl(self.low(s), n.u32()),
            "000_01nnnnnsssddd" => {
                self.cpu.low[d.us()] = self.cpu.lsr::<true>(self.low(s), n.u32())
            }
            "000_10nnnnnsssddd" => {
                self.cpu.low[d.us()] = self.cpu.asr::<true>(self.low(s), n.u32())
            }

            // THUMB.2
            "00011_00nnnsssddd" => self.cpu.low[d.us()] = self.cpu.add(self.low(s), self.low(n)),
            "00011_01nnnsssddd" => self.cpu.low[d.us()] = self.cpu.sub(self.low(s), self.low(n)),
            "00011_10nnnsssddd" => self.cpu.low[d.us()] = self.cpu.add(self.low(s), n.u32()),
            "00011_11nnnsssddd" => self.cpu.low[d.us()] = self.cpu.sub(self.low(s), n.u32()),

            // THUMB.3
            "001_00dddnnnnnnnn" => {
                self.cpu.set_zn(n.u32());
                self.cpu.low[d.us()] = n.u32();
            } // MOV
            "001_01dddnnnnnnnn" => {
                let rd = self.low(d);
                self.cpu.sub(rd, n.u32());
            } // CMP
            "001_10dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.add(self.low(d), n.u32()),
            "001_11dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.sub(self.low(d), n.u32()),

            // THUMB.4
            "010000_oooosssddd" => {
                let rd = self.low(d);
                let rs = self.low(s);
                self.cpu.low[d.us()] = match o {
                    0x0 => self.cpu.and(rd, rs),
                    0x1 => self.cpu.xor(rd, rs),
                    0x2 => {
                        self.add_wait_cycles(1);
                        self.cpu.lsl(rd, rs & 0xFF)
                    }
                    0x3 => {
                        self.add_wait_cycles(1);
                        self.cpu.lsr::<false>(rd, rs & 0xFF)
                    }
                    0x4 => {
                        self.add_wait_cycles(1);
                        self.cpu.asr::<false>(rd, rs & 0xFF)
                    }
                    0x5 => self.cpu.adc(rd, rs, self.cpu.flag(Carry) as u32),
                    0x6 => self.cpu.sbc(rd, rs, self.cpu.flag(Carry) as u32),
                    0x7 => {
                        self.add_wait_cycles(1);
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
                        // TODO proper stall amount
                        self.add_wait_cycles(1);
                        self.cpu.mul(rd, rs)
                    }
                    0xE => self.cpu.bit_clear(rd, rs),
                    _ => self.cpu.not(rs),
                }
            }

            // THUMB.5
            "010001_00dssssddd" => {
                let res = self.reg(d.u32()).wrapping_add(self.reg(s.u32()));
                self.set_reg(d.u32(), res);
            }
            "010001_01dssssddd" => {
                self.cpu.sub(self.reg(d.u32()), self.reg(s.u32()));
            } // CMP
            "010001_10dssssddd" => self.set_reg(d.u32(), self.reg(s.u32())),
            "010001_1101111???" => {
                self.cpu.set_flag(Thumb, false);
                self.set_pc(self.cpu.pc); // Align
            } // BX ARM switch
            "010001_110ssss???" => {
                if !self.reg(s.u32()).is_bit(0) {
                    self.cpu.set_flag(Thumb, false);
                    self.set_pc(self.reg(s.u32()) & !3);
                } else {
                    self.set_pc(self.reg(s.u32()) & !1);
                }
            } // BX
            "010001_111ssss???" => self.set_pc(self.reg(s.u32())), // BLX

            // THUMB.6
            "01001_dddnnnnnnnn" => {
                // LDR has +1I
                self.add_wait_cycles(1);
                self.cpu.low[d.us()] =
                    self.read_word_ldrswp(self.cpu.adj_pc() + (n.u32() << 2), NonSeq)
            }

            // THUMB.7/8
            "0101_ooosssbbbddd" => {
                let rb = self.cpu.low(s);
                let ro = self.cpu.low(b);
                let rd = self.cpu.low(d);
                let addr = rb.wrapping_add(ro);

                match o {
                    0 => self.write_word(addr, rd, NonSeq),        // STR
                    1 => self.write_hword(addr, rd.u16(), NonSeq), // STRH
                    2 => self.write_byte(addr, rd.u8(), NonSeq),   // STRB
                    3 => self.cpu.low[d.us()] = self.read_byte(addr, NonSeq) as i8 as i32 as u32, /* LDSB */
                    4 => self.cpu.low[d.us()] = self.read_word_ldrswp(addr, NonSeq), // LDR
                    5 => self.cpu.low[d.us()] = self.read_hword(addr, NonSeq),       // LDRH
                    6 => self.cpu.low[d.us()] = self.read_byte(addr, NonSeq).u32(),  // LDRB
                    // LDSH, needs special handling for unaligned reads which makes it behave as
                    // LBSB
                    _ if addr.is_bit(0) => {
                        self.cpu.low[d.us()] = self.read_byte(addr, NonSeq) as i8 as i32 as u32
                    }
                    _ => self.cpu.low[d.us()] = self.read_hword(addr, NonSeq) as i16 as i32 as u32,
                }
                if o > 2 {
                    // LDR has +1I
                    self.add_wait_cycles(1);
                }
            }

            // THUMB.9
            "011_oonnnnnbbbddd" => {
                let rb = self.cpu.low(b);
                let rd = self.cpu.low(d);

                match o {
                    0 => self.write_word(rb + (n.u32() << 2), rd, NonSeq), // STR
                    1 => self.cpu.low[d.us()] = self.read_word_ldrswp(rb + (n.u32() << 2), NonSeq), /* LDR */
                    2 => self.write_byte(rb + n.u32(), rd.u8(), NonSeq), // STRB
                    _ => self.cpu.low[d.us()] = self.read_byte(rb + n.u32(), NonSeq).u32(), // LDRB
                }

                if o.is_bit(0) {
                    // LDR has +1I
                    self.add_wait_cycles(1);
                }
            }

            // THUMB.10
            "1000_onnnnnbbbddd" => {
                let rb = self.cpu.low(b);
                let ro = n.u32() << 1; // Step 2
                let rd = self.cpu.low(d);
                let addr = rb + ro;

                if o == 0 {
                    self.write_hword(addr, rd.u16(), NonSeq);
                } else {
                    // LDR has +1I
                    self.add_wait_cycles(1);
                    self.cpu.low[d.us()] = self.read_hword(addr, NonSeq).u32();
                }
            }

            // THUMB.11
            "1001_0dddnnnnnnnn" => {
                self.write_word(self.cpu.sp() + (n.u32() << 2), self.cpu.low(d), NonSeq)
            }
            "1001_1dddnnnnnnnn" => {
                // LDR has +1I
                self.add_wait_cycles(1);
                self.cpu.low[d.us()] =
                    self.read_word_ldrswp(self.cpu.sp() + (n.u32() << 2), NonSeq);
            }

            // THUMB.12
            "1010_0dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.adj_pc() + (n.u32() << 2),
            "1010_1dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.sp() + (n.u32() << 2),

            // THUMB.13
            "10110000_0nnnnnnn" => self.cpu.set_sp(self.cpu.sp() + (n.u32() << 2)),
            "10110000_1nnnnnnn" => self.cpu.set_sp(self.cpu.sp() - (n.u32() << 2)),

            // THUMB.14
            "1011_010brrrrrrrr" => {
                let mut sp = self.cpu.sp();
                let mut kind = NonSeq;
                // PUSH
                if b == 1 {
                    sp -= 4;
                    self.write_word(sp, self.cpu.lr(), kind);
                    kind = Seq;
                }

                for reg in (0..8).rev() {
                    if r.is_bit(reg) {
                        sp -= 4;
                        self.write_word(sp, self.cpu.low[reg.us()], kind);
                        kind = Seq;
                    }
                }
                assert!(kind == Seq);
                self.cpu.set_sp(sp);
            }
            "1011_110brrrrrrrr" => {
                let mut sp = self.cpu.sp();
                let mut kind = NonSeq;
                // POP
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.cpu.low[reg.us()] = self.read_word(sp, kind);
                        sp += 4;
                        kind = Seq;
                    }
                }
                if b == 1 {
                    let pc = self.read_word(sp, kind);
                    self.set_pc(pc);
                    sp += 4;
                    kind = Seq;
                }
                assert!(kind == Seq);
                self.cpu.set_sp(sp);
            }

            // THUMB.15
            "1100_0bbbrrrrrrrr" => {
                // STMIA
                let mut kind = NonSeq;
                let mut base_rlist_addr = None;
                for reg in 0..8 {
                    if r.is_bit(reg) {
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
            }
            "1100_1bbbrrrrrrrr" => {
                // LDMIA
                let mut kind = NonSeq;
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.cpu.low[reg.us()] = self.read_word(self.cpu.low(b), kind);
                        self.cpu.low[b.us()] = self.low(b).wrapping_add(4);
                        kind = Seq;
                    }
                }
                if kind == NonSeq {
                    self.on_empty_rlist(b.u32(), false, true, false);
                }
                self.add_wait_cycles(1);
            }

            // THUMB.16
            "1101_ccccnnnnnnnn" => {
                let nn = (n.u8() as i8 as i32) * 2; // Step 2
                let condition = self.cpu.eval_condition(c);
                if condition {
                    self.set_pc(self.cpu.pc.wrapping_add_signed(nn));
                }
            }

            // THUMB.18
            "11100_nnnnnnnnnnn" => {
                let nn = (n.i10() as i32) * 2; // Step 2
                self.set_pc(self.cpu.pc.wrapping_add_signed(nn));
            }

            // THUMB.19
            "11110_nnnnnnnnnnn" => self
                .cpu
                .set_lr(self.cpu.pc.wrapping_add_signed((n.i10() as i32) << 12)),
            "111t1_nnnnnnnnnnn" => {
                let pc = self.cpu.pc;
                self.set_pc(self.cpu.lr().wrapping_add(n.u32() << 1));
                self.cpu.set_lr(pc - 1);
                self.cpu.set_flag(Thumb, t == 1);
            }

            _ => Self::log_unknown_opcode(inst),
        }
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
