use crate::{
    gga::{
        addr::TM0CNT_H,
        cpu::{Cpu, Interrupt},
        GameGirlAdv,
    },
    numutil::NumExt,
};
use serde::{Deserialize, Serialize};
use std::mem;

const DIVS: [u16; 4] = [1, 64, 256, 1024];

#[derive(Default, Deserialize, Serialize)]
pub struct Timers {
    cycle_counters: [u16; 4],
    pub counters: [u16; 4],
}

impl Timers {
    pub fn step(gg: &mut GameGirlAdv, cycles: u16) {
        let mut overflows = 0;
        for tim in 0..4 {
            let prev_overflows = mem::take(&mut overflows);
            let hi = gg[Self::hi_addr(tim)];
            if !hi.is_bit(7) {
                // Timer off
                continue;
            }

            if tim != 0 && hi.is_bit(2) {
                // Increment on previous timer overflow
                for _ in 0..prev_overflows {
                    overflows += Self::inc_timer(gg, tim) as u16;
                }
            } else {
                // Regular clock counting
                let scaler = DIVS[hi.us() & 3];
                gg.timers.cycle_counters[tim] += cycles;
                while gg.timers.cycle_counters[tim] >= scaler {
                    gg.timers.cycle_counters[tim] -= scaler;
                    overflows += Self::inc_timer(gg, tim) as u16;
                }
            }
        }
    }

    pub fn hi_write(gg: &mut GameGirlAdv, idx: usize, value: u16) {
        let addr = Self::hi_addr(idx);
        let was_on = gg[addr].is_bit(7);
        let is_on = value.is_bit(7);
        if !was_on && is_on {
            gg.timers.counters[idx] = gg[addr - 1];
            gg.timers.cycle_counters[idx] = 0;
        }

        gg[addr] = value
    }

    fn inc_timer(gg: &mut GameGirlAdv, idx: usize) -> bool {
        let new = gg.timers.counters[idx].checked_add(1);
        match new {
            Some(val) => {
                gg.timers.counters[idx] = val;
                false
            }
            None => {
                // Overflow
                let addr = Self::hi_addr(idx);
                // Set to reload value
                gg.timers.counters[idx] = gg[addr - 2];
                // Fire IRQ if enabled
                if gg[addr].is_bit(6) {
                    Cpu::request_interrupt_idx(gg, Interrupt::Timer0 as u16 + idx.u16())
                }
                true
            }
        }
    }

    fn hi_addr(idx: usize) -> u32 {
        TM0CNT_H + (idx.u32() << 2)
    }
}
