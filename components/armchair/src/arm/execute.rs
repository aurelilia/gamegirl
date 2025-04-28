use common::numutil::{NumExt, U32Ext};

use super::{
    decode::{self, *},
    ArmHandler, ArmVisitor,
};
use crate::{
    interface::{Bus, CpuVersion},
    memory::{
        access::{NONSEQ, SEQ},
        Address, RelativeOffset,
    },
    state::{Flag::*, Mode, Register},
    Cpu,
};

impl<S: Bus> Cpu<S> {
    pub fn interpret_arm(&mut self, inst: u32) {
        if !self.check_arm_cond(inst) {
            return;
        }

        let handler = Self::get_interpreter_handler_arm(inst);
        handler(self, ArmInst::of(inst));
    }

    pub fn check_arm_cond(&mut self, inst: u32) -> bool {
        // BLX/CP15 on ARMv5 is a special case: it is encoded with NV.
        let armv5_uncond = S::Version::IS_V5 && (inst.bits(25, 7) == 0b111_1101)
            || (inst.bits(24, 9) == 0b1111_1110);
        self.state.eval_condition(inst.bits(28, 4).u16()) || armv5_uncond
    }

    pub fn get_interpreter_handler_arm(inst: u32) -> ArmHandler<Self> {
        S::Version::ARM.interpreter_lut[decode::arm_inst_to_lookup_idx(inst)]
    }

    pub fn get_cached_handler_arm(inst: u32) -> ArmHandler<Self> {
        (S::Version::ARM.cache_handler_lookup)(ArmInst::of(inst))
    }
}

impl<S: Bus> ArmVisitor for Cpu<S> {
    const IS_V5: bool = S::Version::IS_V5;

    fn arm_unknown_opcode(&mut self, inst: ArmInst) {
        self.und_inst(inst);
    }

    fn arm_swi(&mut self) {
        self.exception_occured(crate::Exception::Swi);
    }

    fn arm_b(&mut self, offset: RelativeOffset) {
        self.relative_jump(offset);
    }

    fn arm_bl(&mut self, offset: RelativeOffset) {
        self.state.set_lr(self.state.pc() - Address::WORD);
        self.relative_jump(offset);
    }

    fn arm_bx(&mut self, n: Register) {
        let rn = self.state[n];
        let is_thumb = rn.is_bit(0);
        self.state.set_flag(Thumb, is_thumb);
        self.set_pc(Address(rn - is_thumb as u32));
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) {
        match src {
            ArmSignedOperandKind::Immediate(offset) => {
                self.state.set_lr(self.state.pc() - Address::WORD);
                self.state.set_flag(Thumb, true);
                self.set_pc(self.state.pc().add_rel(offset));
            }

            ArmSignedOperandKind::Register(reg) => {
                let rn = self.state[reg];
                self.state.set_lr(self.state.pc() - Address::WORD);

                let is_thumb = rn.is_bit(0);
                self.state.set_flag(Thumb, is_thumb);
                self.set_pc(Address(rn - is_thumb as u32));
            }
        }
    }

    fn arm_alu_reg(
        &mut self,
        n: Register,
        d: Register,
        m: Register,
        op: ArmAluOp,
        shift_kind: ArmAluShift,
        shift_operand: ArmOperandKind,
        cpsr: bool,
    ) {
        let carry = self.state.is_flag(Carry);
        let (a, b) = match shift_operand {
            ArmOperandKind::Immediate(imm) => {
                let a = self.state[n];
                let rm = self.state[m];
                (a, self.shifted_op::<true>(rm, shift_kind, imm, cpsr))
            }
            ArmOperandKind::Register(reg) => {
                let a = self.reg_pc4(n);
                let rm = self.reg_pc4(m);
                let shift = self.state[reg] & 0xFF;
                (a, self.shifted_op::<false>(rm, shift_kind, shift, cpsr))
            }
        };
        self.alu_inner(op, a, b, carry, d, cpsr);
    }

    fn arm_alu_imm(
        &mut self,
        n: Register,
        d: Register,
        imm: u32,
        imm_ror: u32,
        op: ArmAluOp,
        cpsr: bool,
    ) {
        let carry = self.state.is_flag(Carry);
        let imm = self.ror::<false>(cpsr, imm, imm_ror);
        self.alu_inner(op, self.state[n], imm, carry, d, cpsr);
    }

    fn arm_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmMulOp,
        cpsr: bool,
    ) {
        let rm = self.state[m];
        let rs = self.state[s];
        let rn = self.state[n];
        let rd = self.state[d];

        let a = rm as u64;
        let b = rs as u64;
        let dhi = rd as u64;
        let dlo = rn as u64;

        let out: u64 = match op {
            ArmMulOp::Mul => rm.wrapping_mul(rs) as u64,
            ArmMulOp::Mla => {
                let r = rm.wrapping_mul(rs);
                let r = r.wrapping_add(rn);
                self.bus.tick(1);
                r as u64
            }
            ArmMulOp::Umaal => a.wrapping_mul(b).wrapping_add(dhi).wrapping_add(dlo),
            ArmMulOp::Umull => {
                self.bus.tick(1);
                a.wrapping_mul(b)
            }
            ArmMulOp::Umlal => {
                self.bus.tick(2);
                a.wrapping_mul(b).wrapping_add(dlo | (dhi << 32))
            }
            ArmMulOp::Smull => {
                self.bus.tick(1);
                (a as i32 as i64).wrapping_mul(b as i32 as i64) as u64
            }
            ArmMulOp::Smlal => {
                // SMLAL
                self.bus.tick(2);
                (a as i32 as i64)
                    .wrapping_mul(b as i32 as i64)
                    .wrapping_add((dlo | (dhi << 32)) as i64) as u64
            }
        };

        let is_mul = op == ArmMulOp::Mul || op == ArmMulOp::Mla;
        if !is_mul {
            self.set_reg(d, (out >> 32).u32());
            self.set_reg(n, out.u32());
        } else {
            self.set_reg(d, out.u32());
        }
        if cpsr {
            let neg_bit = if !is_mul { 63 } else { 31 };
            self.state.set_flag(Zero, out == 0);
            self.state.set_flag(Neg, out.is_bit(neg_bit));
            self.state.set_flag(Carry, false);
        }

        // TODO signed might be wrong
        self.apply_mul_idle_ticks(b as u32, op == ArmMulOp::Smull || op == ArmMulOp::Smlal);
    }

    fn arm_sh_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmShMulOp,
        x_top: bool,
        y_top: bool,
    ) {
        let rm = self.state[m];
        let rs = self.state[s];
        let rn = self.state[n] as i32 as i64;
        let rd = self.state[d] as i32 as i64;

        let a = if x_top { rm.high() } else { rm.low() } as i16 as i64;
        let b = if y_top { rs.high() } else { rs.low() } as i16 as i64;
        let dhi = rd;
        let dlo = rn;

        let out: i64 = match op {
            ArmShMulOp::SmlaXy => {
                let r = a.wrapping_mul(b);
                let res = r.wrapping_add(rn);
                if TryInto::<i32>::try_into(res).is_err() {
                    self.state.set_flag(QClamped, true);
                }
                res
            }
            ArmShMulOp::SmlawYOrSmulwY if x_top => {
                // SMULW
                let r = (rm as i32 as i64).wrapping_mul(b);
                r >> 16
            }
            ArmShMulOp::SmlawYOrSmulwY => {
                // SMLAW
                let r = (rm as i32 as i64).wrapping_mul(b);
                let r = r >> 16;
                let res = r.saturating_add(rn);
                if (r as i32).checked_add(rn as i32).is_none() {
                    self.state.set_flag(QClamped, true);
                }
                res
            }
            ArmShMulOp::SmlalXy => {
                let r = dlo | (dhi << 32);
                r.wrapping_add(a.wrapping_mul(b))
            }
            ArmShMulOp::SmulXy => a.wrapping_mul(b),
        };

        if op == ArmShMulOp::SmlalXy {
            // Don't set high reg on any but SMLAL
            self.set_reg(d, (out >> 32) as u32);
            self.set_reg(n, out as u32);
        } else {
            self.set_reg(d, out as u32);
        }

        // TODO wrong
        self.apply_mul_idle_ticks(b as u32, true);
    }

    fn arm_clz(&mut self, m: Register, d: Register) {
        let count = self.state[m].leading_zeros();
        self.set_reg(d, count);
    }

    fn arm_q(&mut self, n: Register, m: Register, d: Register, op: ArmQOp) {
        let rm = self.state[m] as i32;
        let rn = self.state[n] as i32;
        let value = match op {
            ArmQOp::Qadd => rm.saturating_add(rn),
            ArmQOp::Qsub => rm.saturating_sub(rn),
            ArmQOp::QdAdd => rm.saturating_add(rn.saturating_mul(2)),
            ArmQOp::QdSub => rm.saturating_sub(rn.saturating_mul(2)),
        };
        let checked = match op {
            ArmQOp::Qadd => rm.checked_add(rn),
            ArmQOp::Qsub => rm.checked_sub(rn),
            ArmQOp::QdAdd => rn.checked_mul(2).and_then(|rn| rm.checked_add(rn)),
            ArmQOp::QdSub => rn.checked_mul(2).and_then(|rn| rm.checked_sub(rn)),
        };
        if checked.is_none() {
            self.state.set_flag(QClamped, true);
        }
        self.set_reg(d, value as u32);
    }

    fn arm_msr(&mut self, src: ArmOperandKind, flags: bool, ctrl: bool, spsr: bool) {
        let src = match src {
            ArmOperandKind::Immediate(imm) => imm,
            ArmOperandKind::Register(reg) => self.state[reg],
        };
        let mut dest = if spsr {
            self.state.spsr()
        } else {
            self.state.cpsr()
        };

        if flags {
            dest = (dest & 0x00FF_FFFF) | (src & 0xFF00_0000);
        };
        if ctrl && self.state.mode() != Mode::User {
            dest = (dest & 0xFFFF_FF00) | (src & 0xFF);
        };

        if spsr {
            self.state.set_spsr(dest);
        } else {
            // Thumb flag may not be changed
            dest = dest.set_bit(5, false);
            self.state.set_cpsr(dest);
            self.state.check_if_interrupt(&mut self.bus);
        }
    }

    fn arm_mrs(&mut self, d: Register, spsr: bool) {
        let psr = if spsr {
            self.state.spsr()
        } else {
            self.state.cpsr()
        };
        self.set_reg(d, psr.set_bit(4, true));
    }

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) {
        let mut addr = Address(self.state[n]);
        let offset = match offset {
            ArmLdrStrOperandKind::Immediate(imm) => Address(imm),
            ArmLdrStrOperandKind::Register(reg) => Address(self.state[reg]),
            ArmLdrStrOperandKind::ShiftedRegister { base, shift, by } => {
                let base = self.state[base];
                let addr = self.shifted_op::<true>(base, shift, by, false);
                Address(addr)
            }
        };
        if config.pre {
            addr = addr.add_signed(offset, config.up);
        }

        match config.kind {
            ArmLdrStrKind::LoadByte => {
                let val = self.read::<u8>(addr, NONSEQ).u32();
                self.set_reg(d, val);
            }
            ArmLdrStrKind::LoadSignedByte => {
                let val = self.read::<u8>(addr, NONSEQ) as i8 as i32 as u32;
                self.set_reg(d, val);
            }
            ArmLdrStrKind::StoreByte => self.write::<u8>(addr, self.reg_pc4(d).u8(), NONSEQ),

            ArmLdrStrKind::LoadHalfword => {
                let val = self.read::<u16>(addr, NONSEQ).u32();
                self.set_reg(d, val);
            }
            ArmLdrStrKind::LoadSignedHalfword => {
                let val = self.read_hword_ldrsh(addr, NONSEQ);
                self.set_reg(d, val);
            }
            ArmLdrStrKind::StoreHalfword => self.write::<u16>(addr, self.reg_pc4(d).u16(), NONSEQ),

            ArmLdrStrKind::LoadWord => {
                let val = self.read_word_ldrswp(addr, NONSEQ);
                self.set_reg_allow_switch(d, val);
            }
            ArmLdrStrKind::StoreWord => self.write::<u32>(addr, self.reg_pc4(d), NONSEQ),

            ArmLdrStrKind::LoadDoubleWord => {
                let val = self.read::<u32>(addr, NONSEQ);
                self.set_reg(d, val);
                let val = self.read::<u32>(addr + Address::WORD, SEQ);
                self.set_reg(Register((d.0 + 1) & 15), val);
            }
            ArmLdrStrKind::StoreDoubleWord => {
                self.write::<u32>(addr, self.reg_pc4(d), NONSEQ);
                let value = self.reg_pc4(Register((d.0 + 1) & 15));
                self.write::<u32>(addr + Address::WORD, value, SEQ);
            }
        }

        if !config.pre {
            addr = addr.add_signed(offset, config.up);
        }
        // Edge case: If n == d on an LDR, writeback does nothing
        if config.writeback && (config.kind.is_str() || n != d) {
            self.set_reg(n, addr.0);
        }

        self.state.access_type = NONSEQ;
        if config.kind.is_ldr() {
            // All LDR stall by 1I
            self.bus.tick(1);
        }
    }

    fn arm_ldmstm(&mut self, n: Register, rlist: u16, force_user: bool, config: ArmLdmStmConfig) {
        // Prelude
        let starting_addr = Address(self.state[n]);
        let cpsr = self.state.cpsr();
        if force_user {
            self.state.set_mode(Mode::System);
        }

        // Figure out parameters
        let mut addr = starting_addr;
        let first_register = Register(rlist.trailing_zeros() as u16);
        let last_register = 15u16
            .checked_sub(rlist.leading_zeros() as u16)
            .map(Register);
        let register_count = rlist.count_ones();
        let ending_offset = Address(register_count * 4);
        if !config.up {
            addr = addr.add_signed(Address::WORD, !config.pre);
            addr -= ending_offset;
        }
        let mut kind = NONSEQ;
        let mut set_n = false;

        // Actually copy data
        for reg in Register::from_rlist(rlist) {
            set_n |= reg == n;
            if config.pre {
                addr += Address::WORD
            }
            if !config.ldr && reg == n && (!Self::IS_V5 && reg != first_register) {
                self.set_reg(n, starting_addr.add_signed(ending_offset, config.up).0);
            }

            if config.ldr {
                let val = self.read::<u32>(addr, kind);
                self.set_reg_allow_switch(reg, val);
            } else {
                let val = self.reg_pc4(reg);
                self.write(addr, val, kind);
            }

            kind = SEQ;
            if !config.pre {
                addr += Address::WORD
            }
        }

        // Epilogue
        if force_user {
            self.state.set_cpsr(cpsr);
        }

        let ldr_writeback = Self::IS_V5 && (register_count == 1 || last_register != Some(n));
        if config.writeback && (!config.ldr || !set_n || ldr_writeback) {
            self.set_reg(n, starting_addr.add_signed(ending_offset, config.up).0);
        }

        if kind == NONSEQ {
            self.on_empty_rlist(n, !config.ldr, config.up, config.pre);
        }
        self.state.access_type = NONSEQ;
        if config.ldr {
            // All LDR stall by 1I
            self.bus.tick(1);
        }
    }

    fn arm_swp(&mut self, n: Register, d: Register, m: Register, word: bool) {
        let addr = Address(self.state[n]);
        let reg = self.state[m];
        if word {
            let out = self.read_word_ldrswp(addr, NONSEQ);
            self.set_reg(d, out);
            self.write::<u32>(addr, reg, NONSEQ);
        } else {
            let out = self.read::<u8>(addr, NONSEQ);
            self.set_reg(d, out.u32());
            self.write::<u8>(addr, reg.u8(), NONSEQ);
        }
        self.idle_nonseq();
    }

    fn arm_mrc(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32) {
        if pn != 15 || opc != 0 {
            self.und_inst(pn);
        }

        let value = self.bus.get_cp15(cm, cp, cn);
        if rd.is_pc() {
            let cpsr = self.state.cpsr() & 0x0FFF_FFFF;
            self.state.set_cpsr(cpsr | (value & 0xF000_0000));
        } else {
            self.set_reg(rd, value);
        }
    }

    fn arm_mcr(&mut self, cm: u32, cp: u32, pn: u32, rd: Register, cn: u32, opc: u32) {
        if pn != 15 || opc != 0 {
            self.und_inst(pn);
        }

        let rd = self.reg_pc4(rd);
        self.bus.set_cp15(cm, cp, cn, rd);
    }
}

impl<S: Bus> Cpu<S> {
    fn alu_inner(&mut self, op: ArmAluOp, a: u32, b: u32, c: bool, d: Register, cpsr: bool) {
        let value = match op {
            ArmAluOp::And => self.and(cpsr, a, b),
            ArmAluOp::Eor => self.xor(cpsr, a, b),
            ArmAluOp::Sub => self.sub(cpsr, a, b),
            ArmAluOp::Rsb => self.sub(cpsr, b, a),
            ArmAluOp::Add => self.add(cpsr, a, b),
            ArmAluOp::Adc => self.adc(cpsr, a, b, c as u32),
            ArmAluOp::Sbc => self.sbc(cpsr, a, b, c as u32),
            ArmAluOp::Rsc => self.sbc(cpsr, b, a, c as u32),
            ArmAluOp::Tst => {
                self.and(true, a, b);
                0
            }
            ArmAluOp::Teq => {
                self.xor(true, a, b);
                0
            }
            ArmAluOp::Cmp => {
                self.sub(true, a, b);
                0
            }
            ArmAluOp::Cmn => {
                self.add(true, a, b);
                0
            }
            ArmAluOp::Orr => self.or(cpsr, a, b),
            ArmAluOp::Mov => {
                self.set_nz(cpsr, b);
                b
            }
            ArmAluOp::Bic => self.bit_clear(cpsr, a, b),
            ArmAluOp::Mvn => self.not(cpsr, b),
        };

        if cpsr && d.is_pc() && self.state.mode() != Mode::User && self.state.mode() != Mode::System
        {
            // If S=1, not in user/system mode and the dest is the PC, set CPSR to current
            // SPSR, also flush pipeline if switch to Thumb occurred
            let spsr = self.state.spsr();
            self.state.set_cpsr(spsr);
        }

        if op.should_write() {
            // Only write if needed - might set PC when we should not
            self.set_reg(d, value);
        }
    }

    fn shifted_op<const IMM: bool>(
        &mut self,
        nn: u32,
        op: ArmAluShift,
        shift_amount: u32,
        cpsr: bool,
    ) -> u32 {
        if op == ArmAluShift::Lsl && shift_amount == 0 {
            // Special case: no shift
            nn
        } else {
            match op {
                ArmAluShift::Lsl => self.lsl(cpsr, nn, shift_amount),
                ArmAluShift::Lsr => self.lsr::<IMM>(cpsr, nn, shift_amount),
                ArmAluShift::Asr => self.asr::<IMM>(cpsr, nn, shift_amount),
                ArmAluShift::Ror => self.ror::<IMM>(cpsr, nn, shift_amount),
            }
        }
    }
}
