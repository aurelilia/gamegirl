use common::numutil::NumExt;
use cranelift::prelude::*;

use super::{Condition, InstructionTranslator};
use crate::{access::*, interface::Bus, misc::InstructionKind, RelativeOffset};

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn insert_function_preamble(&mut self) {
        let mut addr = self.current_instruction;
        for instr in &self.ana.instructions {
            if instr.is_branch_target {
                self.instruction_target_blocks
                    .insert(addr, self.builder.create_block());
            }
            addr += self.ana.kind.addr_width();
        }
    }

    pub fn insert_function_exit(&mut self) {}

    pub fn insert_instruction_preamble(
        &mut self,
        wait_time: u64,
        instr_size: Value,
        is_target: bool,
    ) {
        if is_target {
            let block = self.instruction_target_blocks[&self.current_instruction];
            self.builder.ins().jump(block, &[]);
            self.builder.switch_to_block(block);
        }

        self.instructions_since_sync += 1;
        self.wait_time_collected += wait_time as usize;
        if self.instructions_since_sync > 8 || is_target {
            if !is_target {
                self.instructions_since_sync = 0;
            }
            self.synchronize_with_system();
        }

        // Bump PC
        let pc = self.load_pc();
        let pc_new = self.builder.ins().iadd(pc, instr_size);
        self.store_pc(pc_new);
    }

    pub fn relative_jump(&mut self, offset: RelativeOffset) {
        let width = self.ana.kind.width() as i64;
        self.stall_pipeline();

        // Update PC
        let pc = self.load_pc();
        let value = self.builder.ins().iadd_imm(pc, offset.0 as i64 + width);
        self.store_pc(value);

        // Get target block and jump to it
        let target_addr = self
            .current_instruction
            .add_rel(RelativeOffset(offset.0 + self.ana.kind.width() as i32 * 2));
        let block = self.instruction_target_blocks[&target_addr];
        self.builder.ins().jump(block, &[]);
    }

    pub fn stall_pipeline(&mut self) {
        if self.ana.kind == InstructionKind::Arm {
            self.wait_time_collected +=
                self.bus
                    .wait_time::<u32>(&mut self.cpu, self.current_instruction, NONSEQ | CODE)
                    as usize;
            self.wait_time_collected +=
                self.bus
                    .wait_time::<u32>(&mut self.cpu, self.current_instruction, SEQ | CODE)
                    as usize;
        } else {
            self.wait_time_collected +=
                self.bus
                    .wait_time::<u16>(&mut self.cpu, self.current_instruction, NONSEQ | CODE)
                    as usize;
            self.wait_time_collected +=
                self.bus
                    .wait_time::<u16>(&mut self.cpu, self.current_instruction, SEQ | CODE)
                    as usize;
        }
    }

    fn synchronize_with_system(&mut self) {
        // Handle events, make sure we didn't jump somewhere else
        self.bus_handle_events();
        let should_keep_going = self.get_valid();
        let cont_block = self.builder.create_block();
        self.builder.ins().brif(
            should_keep_going,
            cont_block,
            &[],
            self.vals.abort_block,
            &[],
        );
        self.builder.switch_to_block(cont_block);

        // Tick bus
        self.tick_bus(self.wait_time_collected as u64);
        self.wait_time_collected = 0;
    }

    /// Emit code evaluating the given condition, given current CPSR.
    /// Returns non-0 for "run", 0 for "don't".
    pub fn evaluate_condition(&mut self, cond: u16) -> Condition {
        // This condition table is taken from mGBA sources, which are licensed under
        // MPL2 at https://github.com/mgba-emu/mgba
        // Thank you to endrift and other mGBA contributors!
        const COND_MASKS: [u16; 14] = [
            0xF0F0, // EQ [-Z--]
            0x0F0F, // NE [-z--]
            0xCCCC, // CS [--C-]
            0x3333, // CC [--c-]
            0xFF00, // MI [N---]
            0x00FF, // PL [n---]
            0xAAAA, // VS [---V]
            0x5555, // VC [---v]
            0x0C0C, // HI [-zC-]
            0xF3F3, // LS [-Z--] || [--c-]
            0xAA55, // GE [N--V] || [n--v]
            0x55AA, // LT [N--v] || [n--V]
            0x0A05, // GT [Nz-V] || [nz-v]
            0xF5FA, // LE [-Z--] || [Nz-v] || [nz-V]
        ];
        match cond {
            0xE => Condition::RunAlways,
            0xF => Condition::RunNever,
            _ => {
                let cpsr = self.get_cpsr();
                let cond_bits = self.builder.ins().ushr_imm(cpsr, 28);
                let cond_shift = self.builder.ins().ishl(self.consts.one_i32, cond_bits);
                let mask = self
                    .builder
                    .ins()
                    .iconst(types::I32, COND_MASKS[cond.us()] as u64 as i64);
                Condition::RunIf(self.builder.ins().band(mask, cond_shift))
            }
        }
    }

    pub fn tick_bus(&mut self, by: u64) {
        let ticks = self.imm(by as i64, types::I64);
        self.bus_tick(ticks);
    }

    pub fn may_have_invalidated_pc(&mut self) {
        self.instructions_since_sync += 8;
    }

    pub fn imm(&mut self, value: i64, kind: Type) -> Value {
        self.builder.ins().iconst(kind, value)
    }
}
