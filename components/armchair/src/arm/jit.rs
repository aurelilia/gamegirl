use common::numutil::NumExt;
use cranelift::{
    codegen::ir::Inst,
    prelude::{types, InstBuilder},
};

use super::{decode::*, ArmVisitor};
use crate::{
    access::SEQ,
    interface::{Bus, CpuVersion},
    memory::RelativeOffset,
    optimizations::{
        analyze::InstructionAnalysis,
        jit::{Condition, InstructionTranslator},
    },
    state::Register,
    Address, Cpu,
};

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn translate_arm(&mut self, instr: &InstructionAnalysis) {
        let wait = self
            .bus
            .wait_time::<u16>(&mut self.cpu, self.current_instruction, SEQ);
        self.insert_instruction_preamble(wait as u64, self.consts.four_i32, instr.is_branch_target);
        if self.bus.debugger().tracing() {
            let inst = self.imm(instr.instr as i64, types::I32);
            self.call_cpui32(Cpu::<S>::trace_inst::<u32>, inst);
        }

        // BLX/CP15 on ARMv5 is a special case: it is encoded with NV.
        let armv5_uncond = S::Version::IS_V5 && (instr.instr.bits(25, 7) == 0b111_1101)
            || (instr.instr.bits(24, 9) == 0b1111_1110);
        let cond = self.evaluate_condition(instr.instr.bits(28, 4).u16());
        match cond {
            Condition::RunNever if armv5_uncond => self.encode_arm_instruction_run(instr),
            Condition::RunAlways => self.encode_arm_instruction_run(instr),
            Condition::RunNever => (),

            Condition::RunIf(value) => {
                let exec_block = self.builder.create_block();
                let cont_block = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(value, exec_block, &[], cont_block, &[]);

                self.builder.switch_to_block(exec_block);
                self.encode_arm_instruction_run(instr);
                self.builder.ins().jump(cont_block, &[]);
                self.builder.seal_block(exec_block);

                self.builder.switch_to_block(cont_block);
            }
        }
    }

    fn encode_arm_instruction_run(&mut self, instr: &InstructionAnalysis) {
        let inst = ArmInst::of(instr.instr);
        let handle = super::decode::get_instruction_handler(inst, false);
        let implemented = handle(self, inst);
        if !implemented {
            let inst = self.imm(instr.instr as i64, types::I32);
            self.call_cpui32(Cpu::<S>::interpret_arm, inst);
        }
        self.stats.total_instructions += 1;
        self.stats.native_instructions += implemented as usize;
    }
}

impl<S: Bus> ArmVisitor for InstructionTranslator<'_, '_, '_, S> {
    const IS_V5: bool = S::Version::IS_V5;

    type Output = bool;

    fn arm_unknown_opcode(&mut self, inst: ArmInst) -> Self::Output {
        false
    }

    fn arm_swi(&mut self) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn arm_b(&mut self, offset: RelativeOffset) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn arm_bl(&mut self, offset: RelativeOffset) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn arm_bx(&mut self, n: Register) -> Self::Output {
        self.may_have_invalidated_pc();
        false
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) -> Self::Output {
        self.may_have_invalidated_pc();
        false
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
    ) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_alu_imm(
        &mut self,
        n: Register,
        d: Register,
        imm: u32,
        imm_ror: u32,
        op: ArmAluOp,
        cpsr: bool,
    ) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_mul(
        &mut self,
        n: Register,
        s: Register,
        d: Register,
        m: Register,
        op: ArmMulOp,
        cpsr: bool,
    ) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
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
    ) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_clz(&mut self, m: Register, d: Register) -> Self::Output {
        false
    }

    fn arm_q(&mut self, n: Register, m: Register, d: Register, op: ArmQOp) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_msr(
        &mut self,
        src: ArmOperandKind,
        flags: bool,
        ctrl: bool,
        spsr: bool,
    ) -> Self::Output {
        false
    }

    fn arm_mrs(&mut self, d: Register, spsr: bool) -> Self::Output {
        false
    }

    fn arm_ldrstr(
        &mut self,
        n: Register,
        d: Register,
        offset: ArmLdrStrOperandKind,
        config: ArmLdrStrConfig,
    ) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_ldmstm(
        &mut self,
        n: Register,
        rlist: u16,
        force_user: bool,
        config: ArmLdmStmConfig,
    ) -> Self::Output {
        if rlist.is_bit(15) {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_swp(&mut self, n: Register, d: Register, m: Register, word: bool) -> Self::Output {
        if d.is_pc() {
            self.may_have_invalidated_pc();
        }
        false
    }

    fn arm_mrc(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output {
        false
    }

    fn arm_mcr(
        &mut self,
        cm: u32,
        cp: u32,
        pn: u32,
        rd: Register,
        cn: u32,
        opc: u32,
    ) -> Self::Output {
        false
    }
}
