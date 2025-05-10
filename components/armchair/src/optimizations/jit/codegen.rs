use common::numutil::NumExt;
use cranelift::prelude::*;

use super::{Condition, InstructionTranslator};
use crate::{interface::Bus, Cpu};

impl<S: Bus> InstructionTranslator<'_, '_, '_, S> {
    pub fn insert_block_preamble(&mut self) {
        self.call_cpu(Cpu::setup_cpu_state);
    }

    pub fn insert_block_destructor(&mut self) {}

    pub fn insert_instruction_preamble(&mut self, wait_time: u64, instr_size: Value) {
        self.inst_count += 1;
        self.wait_time_collected += wait_time as usize;

        if self.inst_count > 8 {
            self.inst_count = 0;

            // Handle events, make sure we didn't jump somewhere else
            self.call_buscpu(S::handle_events);
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

        // Bump PC
        let pc = self.get_pc();
        let pc_new = self.builder.ins().iadd(pc, instr_size);
        self.set_pc(pc_new);
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
                let cond_bits = self.builder.ins().ishl_imm(cpsr, 28);
                let cond_shift = self.builder.ins().ushr(self.consts.one_i32, cond_bits);
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
        self.call_busi64(S::tick, ticks);
    }

    pub fn may_have_invalidated_pc(&mut self) {
        self.inst_count += 8;
    }

    pub fn imm(&mut self, value: i64, kind: Type) -> Value {
        self.builder.ins().iconst(kind, value)
    }
}
