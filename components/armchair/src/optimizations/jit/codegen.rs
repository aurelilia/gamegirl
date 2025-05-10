use cranelift::prelude::*;

use super::InstructionTranslator;
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
