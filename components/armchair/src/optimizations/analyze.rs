use alloc::vec::Vec;

use common::numutil::NumExt;

use crate::{
    arm::{self, decode::*, ArmInst, ArmVisitor},
    misc::InstructionKind,
    state::{LowRegister, Register},
    thumb::{self, decode::*, ThumbInst, ThumbVisitor},
    Address, RelativeOffset,
};

pub struct InstructionAnalyzer<'s, R: FnMut(Address) -> u32> {
    bus: &'s mut R,
    ana: BlockAnalysis,

    current_pc: Address,
    furthest_branch: Address,
    found_exit: bool,

    last_neg_flag_set: Address,
    last_zero_flag_set: Address,
    last_carry_flag_set: Address,
    last_overflow_flag_set: Address,
}

impl<'s, R: FnMut(Address) -> u32> InstructionAnalyzer<'s, R> {
    pub fn analyze(bus: &'s mut R, entry: Address, kind: InstructionKind) -> BlockAnalysis {
        let mut analyzer = Self {
            bus,
            ana: BlockAnalysis {
                entry,
                exit: Address(0),
                kind,
                pure: true,
                instructions: Vec::new(),
            },

            current_pc: entry,
            furthest_branch: entry,
            found_exit: false,
            last_neg_flag_set: entry,
            last_zero_flag_set: entry,
            last_carry_flag_set: entry,
            last_overflow_flag_set: entry,
        };

        analyzer.analyze_block();
        analyzer.ana
    }

    fn analyze_block(&mut self) {
        while !self.found_exit {
            match self.ana.kind {
                InstructionKind::Arm => {
                    let instr = (self.bus)(self.current_pc - Address::WORD);
                    let inst = ArmInst::of(instr);
                    let mut ana = (arm::decode::get_instruction_handler(inst, false))(self, inst);
                    ana.instr = instr;
                    self.insert_analysis(ana);
                    self.current_pc += Address::WORD;
                }
                InstructionKind::Thumb => {
                    let instr = (self.bus)(self.current_pc - Address::HW).u16();
                    let inst = ThumbInst::of(instr);
                    let mut ana = (thumb::decode::get_instruction_handler(inst))(self, inst);
                    ana.instr = instr as u32;
                    self.insert_analysis(ana);
                    self.current_pc += Address::HW;
                }
            };
            if self.current_pc.on_page_boundary() {
                // Do not go past page boundaries. This serves 2 purposes:
                // - Prevents us from making giant functions when code is malformed
                // - Makes JIT analysis in [super::OptimizationData] possible
                log::debug!("Analysis hit page boundary, aborting.");
                break;
            }
        }

        // Subtract one instruction width from the address to undo the increment at the
        // end of the last loop iteration
        self.ana.exit = self.current_pc - self.ana.kind.addr_width();
    }

    // Used by instruction analysis
    fn uses_nothing(&mut self) -> InstructionAnalysis {
        InstructionAnalysis {
            is_used: true,
            ..Default::default()
        }
    }

    fn uses_carry(&mut self) -> InstructionAnalysis {
        InstructionAnalysis {
            is_used: true,
            uses_carry_flag: true,
            ..Default::default()
        }
    }

    fn uses_all_flags(&mut self) -> InstructionAnalysis {
        InstructionAnalysis {
            is_used: true,
            uses_neg_flag: true,
            uses_zero_flag: true,
            uses_carry_flag: true,
            uses_overflow_flag: true,
            ..Default::default()
        }
    }

    fn unconditional_return(&mut self) -> InstructionAnalysis {
        // If we found an unconditional branch that is past any forward branches, assume
        // that we hit the end of the function
        self.found_exit |= self.current_pc > self.furthest_branch;
        self.uses_all_flags()
    }

    fn branch(&mut self, offset: RelativeOffset) -> InstructionAnalysis {
        let addr = self.current_pc.add_rel(offset);
        if let Some(target) = self.instruction_at_index(addr) {
            target.is_branch_target = true;
            if addr > self.furthest_branch {
                self.furthest_branch = addr;
            }
        }
        self.uses_all_flags()
    }

    fn call_absolute(&mut self, _address: Address) -> InstructionAnalysis {
        self.uses_all_flags()
    }

    fn call_relative(&mut self, _offset: RelativeOffset) -> InstructionAnalysis {
        self.uses_all_flags()
    }

    fn call_register(&mut self, _reg: Register) -> InstructionAnalysis {
        self.uses_all_flags()
    }

    fn sets_nz(&mut self) {
        self.last_neg_flag_set = self.current_pc;
        self.last_zero_flag_set = self.current_pc;
    }

    fn sets_nzc(&mut self) {
        self.last_carry_flag_set = self.current_pc;
        self.sets_nz();
    }

    fn sets_nzco(&mut self) {
        self.last_overflow_flag_set = self.current_pc;
        self.sets_nzc();
    }

    // Internal helpers
    fn insert_analysis(&mut self, analysis: InstructionAnalysis) {
        *self.instruction_at_index(self.current_pc).unwrap() = analysis;
    }

    fn instruction_at_index(&mut self, addr: Address) -> Option<&mut InstructionAnalysis> {
        if addr < self.ana.entry {
            log::debug!("Backwards-to-earlier jump!");
            self.ana.pure = false;
            return None;
        }
        let index = if self.ana.kind == InstructionKind::Thumb {
            ((addr.0 - self.ana.entry.0) >> 1).us()
        } else {
            ((addr.0 - self.ana.entry.0) >> 2).us()
        };
        if self.ana.instructions.len() <= index {
            self.ana
                .instructions
                .resize_with(index + 1, InstructionAnalysis::default);
        }
        Some(&mut self.ana.instructions[index])
    }
}

#[derive(Debug, Clone)]
pub struct BlockAnalysis {
    pub entry: Address,
    pub exit: Address,
    pub kind: InstructionKind,
    pub pure: bool,
    pub instructions: Vec<InstructionAnalysis>,
}

#[derive(Debug, Clone, Default)]
#[allow(unused)]
pub struct InstructionAnalysis {
    pub instr: u32,
    pub is_branch_target: bool,
    pub is_used: bool,
    pub uses_neg_flag: bool,
    pub uses_zero_flag: bool,
    pub uses_carry_flag: bool,
    pub uses_overflow_flag: bool,
}

impl<'s, R: FnMut(Address) -> u32> ThumbVisitor for InstructionAnalyzer<'s, R> {
    type Output = InstructionAnalysis;

    fn thumb_unknown_opcode(&mut self, _inst: ThumbInst) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_alu_imm(
        &mut self,
        _kind: Thumb1Op,
        _d: LowRegister,
        _s: LowRegister,
        _n: u32,
    ) -> Self::Output {
        self.sets_nzc();
        self.uses_nothing()
    }

    fn thumb_2_reg(
        &mut self,
        _kind: Thumb2Op,
        _d: LowRegister,
        _s: LowRegister,
        _n: LowRegister,
    ) -> Self::Output {
        self.sets_nzco();
        self.uses_nothing()
    }

    fn thumb_3(&mut self, _kind: Thumb3Op, _d: LowRegister, _n: u32) -> Self::Output {
        self.sets_nzco();
        self.uses_nothing()
    }

    fn thumb_alu(&mut self, kind: Thumb4Op, _d: LowRegister, _s: LowRegister) -> Self::Output {
        use Thumb4Op::*;
        match kind {
            Adc | Sbc | Neg | Cmp | Cmn => self.sets_nzco(),
            Lsl | Lsr | Asr | Ror => self.sets_nzc(),
            _ => self.sets_nz(),
        }
        match kind {
            Adc | Sbc => self.uses_carry(),
            _ => self.uses_nothing(),
        }
    }

    fn thumb_hi_add(&mut self, _r: (Register, Register)) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_hi_cmp(&mut self, _r: (Register, Register)) -> Self::Output {
        self.sets_nzco();
        self.uses_nothing()
    }

    fn thumb_hi_mov(&mut self, _r: (Register, Register)) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_hi_bx(&mut self, s: Register, blx: bool) -> Self::Output {
        if blx {
            self.call_register(s)
        } else {
            self.unconditional_return()
        }
    }

    fn thumb_ldr6(&mut self, _d: LowRegister, _offset: Address) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_ldrstr78(
        &mut self,
        _op: ThumbStrLdrOp,
        _d: LowRegister,
        _b: LowRegister,
        _o: LowRegister,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_ldrstr9(
        &mut self,
        _op: ThumbStrLdrOp,
        _d: LowRegister,
        _b: LowRegister,
        _offset: Address,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_ldrstr10(
        &mut self,
        _str: bool,
        _d: LowRegister,
        _b: LowRegister,
        _offset: Address,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_str_sp(&mut self, _d: LowRegister, _offset: Address) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_ldr_sp(&mut self, _d: LowRegister, _offset: Address) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_rel_addr(&mut self, _sp: bool, _d: LowRegister, _offset: Address) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_sp_offs(&mut self, _offset: RelativeOffset) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_push(&mut self, _reg_list: u8, _lr: bool) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_pop(&mut self, _reg_list: u8, pc: bool) -> Self::Output {
        if pc {
            self.unconditional_return()
        } else {
            self.uses_nothing()
        }
    }

    fn thumb_stmia(&mut self, _b: LowRegister, _reg_list: u8) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_ldmia(&mut self, _b: LowRegister, _reg_list: u8) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_bcond(&mut self, _cond: u16, offset: RelativeOffset) -> Self::Output {
        self.branch(offset)
    }

    fn thumb_swi(&mut self) -> Self::Output {
        // TODO consider SWI
        self.uses_nothing()
    }

    fn thumb_br(&mut self, offset: RelativeOffset) -> Self::Output {
        self.branch(offset)
    }

    fn thumb_set_lr(&mut self, _offset: RelativeOffset) -> Self::Output {
        self.uses_nothing()
    }

    fn thumb_bl(&mut self, offset: Address, _thumb: bool) -> Self::Output {
        self.call_absolute(offset)
    }
}

impl<'s, R: FnMut(Address) -> u32> ArmVisitor for InstructionAnalyzer<'s, R> {
    const IS_V5: bool = true;
    type Output = InstructionAnalysis;

    fn arm_unknown_opcode(&mut self, _inst: ArmInst) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_swi(&mut self) -> Self::Output {
        // TODO consider SWI
        self.uses_nothing()
    }

    fn arm_b(&mut self, offset: RelativeOffset) -> Self::Output {
        self.branch(offset)
    }

    fn arm_bl(&mut self, offset: RelativeOffset) -> Self::Output {
        self.call_relative(offset)
    }

    fn arm_bx(&mut self, _n: Register) -> Self::Output {
        self.unconditional_return()
    }

    fn arm_blx(&mut self, src: ArmSignedOperandKind) -> Self::Output {
        match src {
            ArmSignedOperandKind::Immediate(offs) => self.call_relative(offs),
            ArmSignedOperandKind::Register(reg) => self.call_register(reg),
        }
    }

    fn arm_alu_reg(
        &mut self,
        _n: Register,
        _d: Register,
        _m: Register,
        op: ArmAluOp,
        _shift_kind: ArmAluShift,
        _shift_operand: ArmOperandKind,
        cpsr: bool,
    ) -> Self::Output {
        use ArmAluOp::*;
        if cpsr {
            match op {
                Adc | Sbc | Cmp | Cmn => self.sets_nzco(),
                _ => self.sets_nzc(),
            }
        }
        match op {
            Adc | Sbc => self.uses_carry(),
            _ => self.uses_nothing(),
        }
    }

    fn arm_alu_imm(
        &mut self,
        _n: Register,
        _d: Register,
        _imm: u32,
        _imm_ror: u32,
        op: ArmAluOp,
        cpsr: bool,
    ) -> Self::Output {
        use ArmAluOp::*;
        if cpsr {
            match op {
                Adc | Sbc | Cmp | Cmn => self.sets_nzco(),
                _ => self.sets_nzc(),
            }
        }
        match op {
            Adc | Sbc => self.uses_carry(),
            _ => self.uses_nothing(),
        }
    }

    fn arm_mul(
        &mut self,
        _n: Register,
        _s: Register,
        _d: Register,
        _m: Register,
        _op: ArmMulOp,
        cpsr: bool,
    ) -> Self::Output {
        if cpsr {
            self.sets_nzc();
        }
        self.uses_nothing()
    }

    fn arm_sh_mul(
        &mut self,
        _n: Register,
        _s: Register,
        _d: Register,
        _m: Register,
        _op: ArmShMulOp,
        _x_top: bool,
        _y_top: bool,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_clz(&mut self, _m: Register, _d: Register) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_q(&mut self, _n: Register, _m: Register, _d: Register, _op: ArmQOp) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_msr(
        &mut self,
        _src: ArmOperandKind,
        _flags: bool,
        _ctrl: bool,
        _spsr: bool,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_mrs(&mut self, _d: Register, _spsr: bool) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_ldrstr(
        &mut self,
        _n: Register,
        _d: Register,
        _offset: ArmLdrStrOperandKind,
        _config: ArmLdrStrConfig,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_ldmstm(
        &mut self,
        _n: Register,
        _rlist: u16,
        _force_user: bool,
        _config: ArmLdmStmConfig,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_swp(&mut self, _n: Register, _d: Register, _m: Register, _word: bool) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_mrc(
        &mut self,
        _cm: u32,
        _cp: u32,
        _pn: u32,
        _rd: Register,
        _cn: u32,
        _opc: u32,
    ) -> Self::Output {
        self.uses_nothing()
    }

    fn arm_mcr(
        &mut self,
        _cm: u32,
        _cp: u32,
        _pn: u32,
        _rd: Register,
        _cn: u32,
        _opc: u32,
    ) -> Self::Output {
        self.uses_nothing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_thumb_simple() {
        // add r1, r5, r0
        // adc r6, r7
        // bx r6
        let mut bus = fake_bus_thumb("1829417E4736");
        let analysis = InstructionAnalyzer::analyze(&mut bus, Address(0), InstructionKind::Thumb);

        assert_eq!(3, analysis.instructions.len());
        assert_eq!(Address(0), analysis.entry);
        assert_eq!(Address(4), analysis.exit);
        assert!(analysis.instructions[1].uses_carry_flag);
    }

    #[test]
    fn analyze_thumb_small_loop() {
        // add r1, r5, r0
        // loop: str r4, [r2, r1]
        // cmp r6, r7
        // beq loop ($-4)
        // bx r6
        let mut bus = fake_bus_thumb("1829505442BED0FE4736");
        let analysis = InstructionAnalyzer::analyze(&mut bus, Address(0), InstructionKind::Thumb);

        assert_eq!(5, analysis.instructions.len());
        assert_eq!(Address(0), analysis.entry);
        assert_eq!(Address(8), analysis.exit);
        assert!(analysis.instructions[1].is_branch_target);
    }

    // Take a hex string and return a fake bus containing it's data..
    fn fake_bus_thumb(data: &str) -> impl FnMut(Address) -> u32 {
        let byte_data: Vec<u8> = (0..data.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&data[i..i + 2], 16).unwrap())
            .collect();
        move |addr: Address| {
            u16::from_be_bytes(
                byte_data[addr.0.us()..(addr.0.us() + 2)]
                    .try_into()
                    .unwrap(),
            )
            .u32()
        }
    }
}
