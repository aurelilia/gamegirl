use crate::{
    gga::{
        addr::TM0CNT_H,
        cpu::{Cpu, Interrupt},
        GameGirlAdv,
    },
    numutil::NumExt,
};
use serde::{Deserialize, Serialize};

const DIVS: [u16; 4] = [1, 64, 256, 1024];

#[derive(Default, Deserialize, Serialize)]
pub struct Timers {
    cycle_counters: [u16; 4],
    pub counters: [u16; 4],
}

impl Timers {
    pub fn step(gg: &mut GameGirlAdv, cycles: u16) {
        let ov = Self::step_idx::<0>(gg, cycles, 0);
        let ov = Self::step_idx::<1>(gg, cycles, ov);
        let ov = Self::step_idx::<2>(gg, cycles, ov);
        Self::step_idx::<3>(gg, cycles, ov);
    }

    #[inline]
    fn step_idx<const TIM: usize>(gg: &mut GameGirlAdv, cycles: u16, prev_overflows: u16) -> u16 {
        let mut overflows = 0;
        let hi = gg[Self::hi_addr::<TIM>()];
        if !hi.is_bit(7) {
            // Timer off
            return 0;
        }

        if TIM != 0 && hi.is_bit(2) {
            // Increment on previous timer overflow
            for _ in 0..prev_overflows {
                overflows += Self::inc_timer::<TIM>(gg) as u16;
            }
        } else {
            // Regular clock counting
            let scaler = DIVS[hi.us() & 3];
            gg.timers.cycle_counters[TIM] += cycles;
            while gg.timers.cycle_counters[TIM] >= scaler {
                gg.timers.cycle_counters[TIM] -= scaler;
                overflows += Self::inc_timer::<TIM>(gg) as u16;
            }
        }
        overflows
    }

    pub fn hi_write<const TIM: usize>(gg: &mut GameGirlAdv, value: u16) {
        let addr = Self::hi_addr::<TIM>();
        let was_on = gg[addr].is_bit(7);
        let is_on = value.is_bit(7);
        if !was_on && is_on {
            gg.timers.counters[TIM] = gg[addr - 2];
            gg.timers.cycle_counters[TIM] = 0;
        }

        gg[addr] = value
    }

    #[inline]
    fn inc_timer<const TIM: usize>(gg: &mut GameGirlAdv) -> bool {
        let new = gg.timers.counters[TIM].checked_add(1);
        match new {
            Some(val) => {
                gg.timers.counters[TIM] = val;
                false
            }
            None => {
                // Overflow
                let addr = Self::hi_addr::<TIM>();
                // Set to reload value
                gg.timers.counters[TIM] = gg[addr - 2];
                // Fire IRQ if enabled
                if gg[addr].is_bit(6) {
                    Cpu::request_interrupt_idx(gg, Interrupt::Timer0 as u16 + TIM.u16())
                }
                true
            }
        }
    }

    #[inline]
    fn hi_addr<const TIM: usize>() -> u32 {
        TM0CNT_H + (TIM.u32() << 2)
    }
}
