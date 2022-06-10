use crate::gga::cpu::registers::Flag::*;
use crate::gga::GameGirlAdv;
use crate::numutil::NumExt;
use bitmatch::bitmatch;

impl GameGirlAdv {
    #[bitmatch]
    pub fn execute_inst_thumb(&mut self, inst: u16) {
        #[bitmatch]
        match inst {
            // THUMB.1
            "000_00nnnnnsssddd" => self.cpu.low[d.us()] = self.cpu.lsl(self.low(s), n.u32()),
            "000_01nnnnnsssddd" => self.cpu.low[d.us()] = self.cpu.lsr(self.low(s), n.u32()),
            "000_10nnnnnsssddd" => self.cpu.low[d.us()] = self.cpu.asr(self.low(s), n.u32()),

            // THUMB.2
            "00011_00nnnsssddd" => self.cpu.low[d.us()] = self.cpu.add(self.low(s), self.low(n), 0),
            "00011_01nnnsssddd" => self.cpu.low[d.us()] = self.cpu.sub(self.low(s), self.low(n), 0),
            "00011_10nnnsssddd" => self.cpu.low[d.us()] = self.cpu.add(self.low(s), n.u32(), 0),
            "00011_11nnnsssddd" => self.cpu.low[d.us()] = self.cpu.sub(self.low(s), n.u32(), 0),

            // THUMB.3
            "001_00dddnnnnnnnn" => {
                self.cpu.set_zn(n.u32());
                self.cpu.low[d.us()] = n.u32();
            } // MOV
            "001_01dddnnnnnnnn" => {
                let rd = self.low(d);
                self.cpu.sub(rd, n.u32(), 0);
            } // CMP
            "001_10dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.add(self.low(d), n.u32(), 0),
            "001_11dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.sub(self.low(d), n.u32(), 0),

            // THUMB.4
            "010000_oooosssddd" => {
                let rd = self.low(d);
                let rs = self.low(s);
                self.cpu.low[d.us()] = match o {
                    0x0 => self.cpu.and(rd, rs),
                    0x1 => self.cpu.xor(rd, rs),
                    0x2 => self.cpu.lsl(rd, rs & 0xFF),
                    0x3 => self.cpu.lsr(rd, rs & 0xFF),
                    0x4 => self.cpu.asr(rd, rs & 0xFF),
                    0x5 => self.cpu.add(rd, rs, self.cpu.flag(Carry) as u32),
                    0x6 => self.cpu.sub(rd, rs, self.cpu.flag(Carry) as u32),
                    0x7 => self.cpu.ror(rd, rs & 0xFF),
                    0x8 => {
                        // TST
                        self.cpu.and(rd, rs);
                        rd
                    }
                    0x9 => self.cpu.neg(rs),
                    0xA => {
                        // CMP
                        self.cpu.sub(rd, rs, 0);
                        rd
                    }
                    0xB => {
                        // CMN
                        self.cpu.add(rd, rs, 0);
                        rd
                    }
                    0xC => self.cpu.or(rd, rs),
                    0xD => self.cpu.mul(rd, rs),
                    0xE => self.cpu.bit_clear(rd, rs),
                    _ => self.cpu.not(rs),
                }
            }

            // THUMB.5
            "010001_00dssssddd" => {
                let res = self.cpu.add(self.reg(d.u32()), self.reg(s.u32()), 0);
                self.cpu.set_reg(d, res);
            }
            "010001_01dssssddd" => {
                self.cpu.sub(self.reg(d.u32()), self.reg(s.u32()), 0);
            } // CMP
            "010001_10dssssddd" => self.cpu.set_reg(d, self.reg(s.u32())),
            "010001_1101111???" => self.cpu.set_flag(Thumb, false), // BX ARM switch
            "010001_110ssss???" => self.cpu.set_pc(self.reg(s.u32())), // BX
            "010001_111ssss???" => self.cpu.set_pc(self.reg(s.u32())), // BLX

            // THUMB.6
            "01001_dddnnnnnnnn" => {
                self.cpu.low[d.us()] = self.read_word(self.cpu.adj_pc() + n.u32())
            }

            // THUMB.7/8
            "0101_ooosssbbbddd" => {
                let rb = self.cpu.low(s);
                let ro = self.cpu.low(b);
                let rd = self.cpu.low(d);
                let addr = rb + ro;

                match o {
                    0 => self.write_word(addr, rd),        // STR
                    1 => self.write_hword(addr, rd.u16()), // STRH
                    2 => self.write_byte(addr, rd.u8()),   // STRB
                    3 => self.cpu.low[d.us()] = self.read_byte(addr) as i8 as i32 as u32, // LDSB
                    4 => self.cpu.low[d.us()] = self.read_word(addr), // LDR
                    5 => self.cpu.low[d.us()] = self.read_hword(addr).u32(), // LDRH
                    6 => self.cpu.low[d.us()] = self.read_byte(addr).u32(), // LDRB
                    _ => self.cpu.low[d.us()] = self.read_hword(addr) as i16 as i32 as u32, // LDSH
                }
            }

            // THUMB.9
            "011_oonnnnnbbbddd" => {
                let rb = self.cpu.low(b);
                let rd = self.cpu.low(d);

                match o {
                    0 => self.write_word(rb + (n.u32() << 2), rd), // STR
                    1 => self.cpu.low[d.us()] = self.read_word(rb + (n.u32() << 2)), // LDR
                    2 => self.write_byte(rb + n.u32(), rd.u8()),   // STRB
                    _ => self.cpu.low[d.us()] = self.read_byte(rb + n.u32()).u32(), // LDRB
                }
            }

            // THUMB.10
            "1000_onnnnnbbbddd" => {
                let rb = self.cpu.low(b);
                let ro = n.u32() << 1; // Step 2
                let rd = self.cpu.low(d);
                let addr = rb + ro;

                if o == 0 {
                    self.write_hword(addr, rd.u16());
                } else {
                    self.cpu.low[d.us()] = self.read_hword(addr).u32();
                }
            }

            // THUMB.11
            "1001_0dddnnnnnnnn" => self.write_word(self.cpu.sp() + n.u32(), self.cpu.low(d)),
            "1001_1dddnnnnnnnn" => self.cpu.low[d.us()] = self.read_word(self.cpu.sp() + n.u32()),

            // THUMB.12
            "1010_0dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.adj_pc() + (n.u32() << 2),
            "1010_1dddnnnnnnnn" => self.cpu.low[d.us()] = self.cpu.sp() + (n.u32() << 2),

            // THUMB.13
            "10110000_0nnnnnnn" => self.cpu.set_sp(self.cpu.sp() + (n.u32() << 2)),
            "10110000_1nnnnnnn" => self.cpu.set_sp(self.cpu.sp() - (n.u32() << 2)),

            // THUMB.14
            "1011_010brrrrrrrr" => {
                let mut sp = self.cpu.sp();
                // PUSH
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.write_word(sp, self.cpu.low[reg.us()]);
                        sp -= 4;
                    }
                }
                if b == 1 {
                    self.write_word(sp, self.cpu.lr());
                    sp -= 4;
                }
                self.cpu.set_sp(sp);
            }
            "1011_110brrrrrrrr" => {
                let mut sp = self.cpu.sp();
                // POP
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.cpu.low[reg.us()] = self.read_word(sp);
                        sp -= 4;
                    }
                }
                if b == 1 {
                    self.cpu.set_pc(self.read_word(sp));
                    sp -= 4;
                }
                self.cpu.set_sp(sp);
            }

            // THUMB.15
            "1100_0bbbrrrrrrrr" => {
                // STMIA
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.write_word(self.cpu.low(b), self.cpu.low[reg.us()]);
                        self.cpu.low[b.us()] += 4;
                    }
                }
            }
            "1100_1bbbrrrrrrrr" => {
                // LDMIA
                for reg in 0..8 {
                    if r.is_bit(reg) {
                        self.cpu.low[reg.us()] = self.read_word(self.cpu.low(b));
                        self.cpu.low[b.us()] += 4;
                    }
                }
            }

            // THUMB.16
            "1101_ccccnnnnnnnn" => {
                let nn = (n.u8() as i8 as i32) * 2; // Step 2
                let condition = self.cpu.eval_condition(c);
                if condition {
                    self.cpu.set_pc(self.cpu.adj_pc().wrapping_add_signed(nn));
                }
            }

            // THUMB.18
            "11100_nnnnnnnnnnn" => {
                let nn = (n as i16 as i32) * 2; // Step 2
                self.cpu.set_pc(self.cpu.adj_pc().wrapping_add_signed(nn));
            }

            // THUMB.19
            "11110_nnnnnnnnnnn" => self.cpu.set_lr(self.cpu.pc + 4 + (n.u32() << 12)),
            "111t0_nnnnnnnnnnn" => {
                self.cpu.set_lr((self.cpu.pc + 2) | 1);
                self.cpu.set_pc(self.cpu.lr() + (n.u32() << 1));
                self.cpu.set_flag(Thumb, t == 1);
            }

            _ => Self::log_unknown_opcode(inst),
        }
    }
}
