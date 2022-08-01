// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use common::numutil::NumExt;

use crate::{
    cpu::{data, data::NAMES, DReg, DReg::*, Flag::*, Reg, Reg::*},
    io::addr::{IE, IF, KEY1},
    GameGirl,
};

const EXT: u8 = 0xCB;

const MATH: [fn(&mut GameGirl, u8) -> u8; 8] = [
    |gg, v| gg.add(gg.cpu.reg(A).u16(), v.u16(), 0, true).u8(),
    |gg, v| {
        gg.add(
            gg.cpu.reg(A).u16(),
            v.u16(),
            gg.cpu.flag(Carry) as u16,
            true,
        )
        .u8()
    },
    |gg, v| gg.sub(gg.cpu.reg(A).u16(), v.u16(), 0, true).u8(),
    |gg, v| {
        gg.sub(
            gg.cpu.reg(A).u16(),
            v.u16(),
            gg.cpu.flag(Carry) as u16,
            true,
        )
        .u8()
    },
    |gg, v| {
        let val = gg.cpu.reg(A) & v;
        gg.cpu
            .set_reg(F, HalfCarry.mask() + (Zero.from(val) ^ Zero.mask()));
        val
    },
    |gg, v| gg.z_flag_only(gg.cpu.reg(A) ^ v),
    |gg, v| gg.z_flag_only(gg.cpu.reg(A) | v),
    |gg, v| {
        gg.sub(gg.cpu.reg(A).u16(), v.u16(), 0, true);
        gg.cpu.reg(A)
    },
];

/// An instruction that can be executed by the CPU.
#[derive(Copy, Clone)]
pub struct Inst(u8, u8);

impl Inst {
    /// Produce the name of the instruction, with immediate
    /// parameters replaced with their actual value.
    pub fn formatted_name(&self, arg: u16) -> String {
        let base = self.get(&data::NAMES, &data::NAMES_EXT);
        let base = base.replace("a8", &format!("FF{:02X}", arg & 0xFF));
        let base = base.replace("a16", &format!("{:04X}", arg));
        let base = base.replace("d8", &format!("d{:02X}", arg & 0xFF));
        let base = base.replace("d16", &format!("d{:04X}", arg));
        let base = base.replace("s8", &format!("d{:02X}", arg.u8() as i8));
        base
    }

    pub fn size(&self) -> u8 {
        self.get_(&data::SIZE, data::SIZE_EXT)
    }

    pub fn inc_pc(&self) -> bool {
        self.get_(&data::INC_PC, data::INC_PC_EXT)
    }

    fn get<T: Copy>(self, reg: &[T], ext: &[T]) -> T {
        match self.0 {
            EXT => ext[self.1 as usize],
            _ => reg[self.0 as usize],
        }
    }

    fn get_<T: Copy>(self, reg: &[T], ext: T) -> T {
        match self.0 {
            EXT => ext,
            _ => reg[self.0 as usize],
        }
    }
}

pub(crate) fn get_next(gg: &mut GameGirl) -> Inst {
    let first = gg.read8(gg.cpu.pc);
    let inst = match first {
        EXT => Inst(first, gg.arg8()),
        _ => Inst(first, 0),
    };

    if common::TRACING {
        println!(
            "PC={:04X}, SP={:04X}, SPV={:04X}, AF={:04X}, BC={:04X}, DE={:4X}, HL={:04X}, I={}",
            gg.cpu.pc,
            gg.cpu.sp,
            gg.get16(gg.cpu.sp),
            gg.cpu.dreg(AF),
            gg.cpu.dreg(BC),
            gg.cpu.dreg(DE),
            gg.cpu.dreg(HL),
            NAMES[inst.0.us()]
        );
    }

    inst
}

pub fn get_at(gg: &GameGirl, addr: u16) -> Inst {
    Inst(gg.get8(addr), gg.get8(addr + 1))
}

const MATH_REGS: [Reg; 8] = [B, C, D, E, H, L, A, A];

pub(super) fn execute(gg: &mut GameGirl, inst: Inst) -> bool {
    const BDH: [Reg; 3] = [B, D, H];
    const CELA: [Reg; 4] = [C, E, L, A];
    const BCDEHLAF: [DReg; 4] = [BC, DE, HL, AF];

    let reg = ((inst.0 as usize) >> 4) & 3;
    match inst.0 {
        // -----------------------------------
        // 0x00 - 0x3F
        // -----------------------------------
        0x00 => (),
        0x10 if gg[KEY1].is_bit(0) => gg.switch_speed(),
        0x20 if !gg.cpu.flag(Zero) => return jr(gg),
        0x30 if !gg.cpu.flag(Carry) => return jr(gg),

        0x01 | 0x11 | 0x21 => {
            let val = gg.arg16();
            gg.cpu.set_dreg(BCDEHLAF[reg], val);
        }
        0x31 => gg.cpu.sp = gg.arg16(),

        0x02 => gg.write8(gg.cpu.dreg(BC), gg.cpu.reg(A)),
        0x12 => gg.write8(gg.cpu.dreg(DE), gg.cpu.reg(A)),
        0x22 => {
            let addr = gg.mod_ret_hl(1);
            gg.write8(addr, gg.cpu.reg(A));
        }
        0x32 => {
            let addr = gg.mod_ret_hl(-1);
            gg.write8(addr, gg.cpu.reg(A));
        }

        0x03 | 0x13 | 0x23 => {
            gg.cpu
                .set_dreg(BCDEHLAF[reg], gg.cpu.dreg(BCDEHLAF[reg]).wrapping_add(1));
            gg.advance_clock(1);
        }
        0x33 => {
            gg.cpu.sp = gg.cpu.sp.wrapping_add(1);
            gg.advance_clock(1);
        }

        0x04 | 0x14 | 0x24 => {
            let val = gg.add(gg.cpu.reg(BDH[reg]).u16(), 1, 0, false).u8();
            gg.cpu.set_reg(BDH[reg], val);
        }
        0x34 => {
            let addr = gg.cpu.dreg(HL);
            let value = gg.read8(addr);
            let val = gg.add(value.u16(), 1, 0, false).u8();
            gg.write8(addr, val);
        }

        0x05 | 0x15 | 0x25 => {
            let val = gg.sub(gg.cpu.reg(BDH[reg]).u16(), 1, 0, false).u8();
            gg.cpu.set_reg(BDH[reg], val);
        }
        0x35 => {
            let addr = gg.cpu.dreg(HL);
            let value = gg.read8(addr);
            let val = gg.sub(value.u16(), 1, 0, false).u8();
            gg.write8(addr, val);
        }

        0x06 | 0x16 | 0x26 => {
            let value = gg.arg8();
            gg.cpu.set_reg(BDH[reg], value);
        }
        0x36 => {
            let value = gg.arg8();
            gg.write8(gg.cpu.dreg(HL), value);
        }

        0x07 => {
            let val = gg.rlc(gg.cpu.reg(A), false);
            gg.cpu.set_reg(A, val);
        }
        0x17 => {
            let val = gg.rl(gg.cpu.reg(A), false);
            gg.cpu.set_reg(A, val);
        }
        0x27 => {
            // i hate DAA
            let mut a = gg.cpu.reg(A).u16();
            if !gg.cpu.flag(Negative) {
                if gg.cpu.flag(Carry) || a > 0x99 {
                    a += 0x60;
                    gg.cpu.set_fl(Carry, true);
                }
                if gg.cpu.flag(HalfCarry) || (a & 0x0F) > 0x09 {
                    a += 0x06;
                }
            } else {
                if gg.cpu.flag(Carry) {
                    a = a.wrapping_sub(0x60);
                }
                if gg.cpu.flag(HalfCarry) {
                    a = a.wrapping_sub(0x06) & 0xFF;
                }
            }

            gg.cpu.set_fl(Zero, (a & 0xFF) == 0);
            gg.cpu.set_fl(HalfCarry, false);
            gg.cpu.set_reg(A, a.u8());
        }
        0x37 => gg
            .cpu
            .set_reg(F, (gg.cpu.reg(F) & Zero.mask()) + 0b0001_0000),

        0x08 => {
            let addr = gg.arg16();
            gg.write16(addr, gg.cpu.sp);
        }
        0x18 => return jr(gg),
        0x28 if gg.cpu.flag(Zero) => return jr(gg),
        0x38 if gg.cpu.flag(Carry) => return jr(gg),

        0x09 | 0x19 | 0x29 => gg.add_16_hl(gg.cpu.dreg(BCDEHLAF[reg])),
        0x39 => gg.add_16_hl(gg.cpu.sp),

        0x0A => {
            let value = gg.read8(gg.cpu.dreg(BC));
            gg.cpu.set_reg(A, value);
        }
        0x1A => {
            let value = gg.read8(gg.cpu.dreg(DE));
            gg.cpu.set_reg(A, value);
        }
        0x2A => {
            let addr = gg.mod_ret_hl(1);
            let value = gg.read8(addr);
            gg.cpu.set_reg(A, value);
        }
        0x3A => {
            let addr = gg.mod_ret_hl(-1);
            let value = gg.read8(addr);
            gg.cpu.set_reg(A, value);
        }

        0x0B | 0x1B | 0x2B => {
            gg.cpu
                .set_dreg(BCDEHLAF[reg], gg.cpu.dreg(BCDEHLAF[reg]).wrapping_sub(1));
            gg.advance_clock(1);
        }
        0x3B => {
            gg.advance_clock(1);
            gg.cpu.sp = gg.cpu.sp.wrapping_sub(1);
        }

        0x0C | 0x1C | 0x2C | 0x3C => {
            let val = gg.add(gg.cpu.reg(CELA[reg]).u16(), 1, 0, false).u8();
            gg.cpu.set_reg(CELA[reg], val);
        }
        0x0D | 0x1D | 0x2D | 0x3D => {
            let val = gg.sub(gg.cpu.reg(CELA[reg]).u16(), 1, 0, false).u8();
            gg.cpu.set_reg(CELA[reg], val);
        }
        0x0E | 0x1E | 0x2E | 0x3E => {
            let value = gg.arg8();
            gg.cpu.set_reg(CELA[reg], value);
        }

        0x0F => {
            let val = gg.rrc(gg.cpu.reg(A), false);
            gg.cpu.set_reg(A, val);
        }
        0x1F => {
            let val = gg.rr(gg.cpu.reg(A), false);
            gg.cpu.set_reg(A, val);
        }
        0x2F => {
            gg.cpu.set_fl(Negative, true);
            gg.cpu.set_fl(HalfCarry, true);
            gg.cpu.set_reg(A, gg.cpu.reg(A) ^ 0xFF);
        }
        0x3F => {
            gg.cpu.set_fl(Negative, false);
            gg.cpu.set_fl(HalfCarry, false);
            gg.cpu.set_fl(Carry, !gg.cpu.flag(Carry));
        }

        // -----------------------------------
        // 0x40 - 0x7F
        // -----------------------------------
        0x76 if !gg.cpu.ime && (gg[IF] & gg[IE] & 0x1F) != 0 => gg.cpu.halt_bug = true,
        0x76 => {
            // HALT: Advance until IF != 0
            while gg[IF] & 0x1F == 0 {
                gg.advance_clock(4);
            }
        }
        0x40..=0x7F => {
            let reg = (inst.0 - 0x40) >> 3;
            match reg {
                6 => {
                    let addr = gg.cpu.dreg(HL);
                    reg_set(gg, inst.0, |gg, v| gg.write8(addr, v));
                }

                _ => reg_set(gg, inst.0, |gg, v| {
                    gg.cpu.set_reg(MATH_REGS[reg as usize], v);
                }),
            }
        }

        // -----------------------------------
        // 0x80 - 0xBF
        // -----------------------------------
        0x80..=0xBF => {
            let op = (inst.0 - 0x80) >> 3;
            reg_set(gg, inst.0, |gg, v| {
                let val = MATH[op as usize](gg, v);
                gg.cpu.set_reg(A, val);
            });
        }

        // -----------------------------------
        // 0xC0 - 0xFF
        // -----------------------------------
        0xC0 if !gg.cpu.flag(Zero) => return ret_5(gg),
        0xD0 if !gg.cpu.flag(Carry) => return ret_5(gg),
        0xE0 => {
            let addr = 0xFF00 + gg.arg8().u16();
            gg.write8(addr, gg.cpu.reg(A));
        }
        0xF0 => {
            let addr = 0xFF00 + gg.arg8().u16();
            let value = gg.read8(addr);
            gg.cpu.set_reg(A, value);
        }

        0xC1 | 0xD1 | 0xE1 | 0xF1 => {
            let val = gg.pop_stack();
            gg.cpu.set_dreg(BCDEHLAF[reg], val);
        }

        0xC2 if !gg.cpu.flag(Zero) => return jp(gg),
        0xD2 if !gg.cpu.flag(Carry) => return jp(gg),
        0xE2 => gg.write8(0xFF00 + gg.cpu.reg(C).u16(), gg.cpu.reg(A)),
        0xF2 => {
            let value = gg.read8(0xFF00 + gg.cpu.reg(C).u16());
            gg.cpu.set_reg(A, value);
        }

        0xC3 => return jp(gg),
        0xF3 => gg.cpu.ime = false,

        0xC4 if !gg.cpu.flag(Zero) => return call(gg),
        0xD4 if !gg.cpu.flag(Carry) => return call(gg),

        0xC5 | 0xD5 | 0xE5 | 0xF5 => {
            let val = gg.cpu.dreg(BCDEHLAF[reg]);
            gg.advance_clock(1);
            gg.push_stack(val);
        }

        _ if inst.0 & 0x0F == 0x06 || inst.0 & 0x0F == 0x0E => {
            let idx = (inst.0 - 0xC6) / 8;
            let imm = gg.arg8();
            let val = MATH[idx as usize](gg, imm);
            gg.cpu.set_reg(A, val);
        }

        _ if inst.0 & 0x0F == 0x07 || inst.0 & 0x0F == 0x0F => {
            gg.advance_clock(1);
            let idx = inst.0 - 0xC7;
            gg.push_stack(gg.cpu.pc + 1);
            gg.cpu.pc = idx.u16();
            return false;
        }

        0xC8 if gg.cpu.flag(Zero) => {
            gg.advance_clock(1);
            return ret(gg);
        }
        0xD8 if gg.cpu.flag(Carry) => {
            gg.advance_clock(1);
            return ret(gg);
        }
        0xE8 => {
            gg.cpu.sp = gg.add_sp();
            gg.advance_clock(2); // Internal delay
        }
        0xF8 => {
            let val = gg.add_sp();
            gg.cpu.set_dreg(HL, val);
            gg.advance_clock(1); // Internal delay
        }

        0xC9 => return ret(gg),
        0xD9 => {
            gg.cpu.ime = true;
            return ret(gg);
        }
        0xE9 => {
            gg.cpu.pc = gg.cpu.dreg(HL);
            return false;
        }
        0xF9 => {
            gg.cpu.sp = gg.cpu.dreg(HL);
            gg.advance_clock(1); // Internal delay
        }

        0xCA if gg.cpu.flag(Zero) => return jp(gg),
        0xDA if gg.cpu.flag(Carry) => return jp(gg),
        0xEA => {
            let addr = gg.arg16();
            gg.write8(addr, gg.cpu.reg(A));
        }
        0xFA => {
            let addr = gg.arg16();
            let value = gg.read8(addr);
            gg.cpu.set_reg(A, value);
        }

        0xCB => execute_ext(gg, inst.1),
        0xFB => gg.cpu.ime = true,

        0xCC if gg.cpu.flag(Zero) => return call(gg),
        0xDC if gg.cpu.flag(Carry) => return call(gg),
        0xCD => return call(gg),

        0x20 | 0x30 | 0x28 | 0x38 | 0xC0 | 0xD0 | 0xC8 | 0xD8 => gg.advance_clock(1), /* Internal delay on failed jump */
        0xC2 | 0xD2 | 0xC4 | 0xD4 | 0xCA | 0xDA | 0xCC | 0xDC => gg.advance_clock(2), /* Internal delay on failed jump */
        _ => (),
    }
    inst.inc_pc()
}

#[must_use]
fn jr(gg: &mut GameGirl) -> bool {
    gg.cpu.pc = gg.cpu.pc.wrapping_add_signed((gg.arg8() as i8) as i16 + 2);
    pc_changed_inst(gg)
}

#[must_use]
fn jp(gg: &mut GameGirl) -> bool {
    gg.cpu.pc = gg.arg16();
    pc_changed_inst(gg)
}

#[must_use]
fn ret(gg: &mut GameGirl) -> bool {
    gg.cpu.pc = gg.pop_stack();
    pc_changed_inst(gg)
}

#[must_use]
fn ret_5(gg: &mut GameGirl) -> bool {
    gg.advance_clock(1);
    ret(gg)
}

#[must_use]
fn call(gg: &mut GameGirl) -> bool {
    let prev_pc = gg.cpu.pc;
    gg.cpu.pc = gg.arg16();
    gg.advance_clock(1); // Internal delay
    gg.push_stack(prev_pc + 3);
    false
}

fn pc_changed_inst(gg: &mut GameGirl) -> bool {
    gg.advance_clock(1); // Internal delay
    false
}

fn execute_ext(gg: &mut GameGirl, ext: u8) {
    match ext & 0xF8 {
        0x00 => reg_ext::<true, _>(gg, ext, |gg, v| gg.rlc(v, true)),
        0x08 => reg_ext::<true, _>(gg, ext, |gg, v| gg.rrc(v, true)),
        0x10 => reg_ext::<true, _>(gg, ext, |gg, v| gg.rl(v, true)),
        0x18 => reg_ext::<true, _>(gg, ext, |gg, v| gg.rr(v, true)),
        0x20 => reg_ext::<true, _>(gg, ext, GameGirl::sla),
        0x28 => reg_ext::<true, _>(gg, ext, GameGirl::sra),
        0x30 => reg_ext::<true, _>(gg, ext, GameGirl::swap),
        0x38 => reg_ext::<true, _>(gg, ext, GameGirl::srl),
        _ => {
            let bit = ((ext & 0b0011_1000) >> 3).u16();
            match ext & 0b1100_0000 {
                0b0100_0000 => reg_ext::<false, _>(gg, ext, |gg, v| gg.bit(v, bit)),
                0b1000_0000 => reg_ext::<true, _>(gg, ext, |_, v| v.set_bit(bit, false).u8()),
                0b1100_0000 => reg_ext::<true, _>(gg, ext, |_, v| v.set_bit(bit, true).u8()),
                _ => panic!("Match statement is wrong?"),
            }
        }
    }
}

fn reg_ext<const ADV: bool, T: FnOnce(&mut GameGirl, u8) -> u8>(
    gg: &mut GameGirl,
    op: u8,
    func: T,
) {
    let reg = op & 0x07;
    if reg == 6 {
        let addr = gg.cpu.dreg(HL);
        let value = gg.read8(addr);
        let value = func(gg, value);
        if ADV {
            gg.advance_clock(1);
        }
        gg.set8(addr, value);
    } else {
        let reg = MATH_REGS[reg as usize];
        let value = gg.cpu.reg(reg);
        let value = func(gg, value);
        gg.cpu.set_reg(reg, value);
    }
}

fn reg_set<T: FnOnce(&mut GameGirl, u8)>(gg: &mut GameGirl, op: u8, func: T) {
    let reg = op & 0x07;
    if reg == 6 {
        let addr = gg.cpu.dreg(HL);
        let value = gg.read8(addr);
        func(gg, value);
    } else {
        let reg = MATH_REGS[reg as usize];
        let value = gg.cpu.reg(reg);
        func(gg, value);
    }
}
