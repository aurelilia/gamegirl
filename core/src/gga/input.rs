use crate::{
    common::Button,
    gga::{
        addr::{KEYCNT, KEYINPUT},
        cpu::{Cpu, Interrupt},
        GameGirlAdv,
    },
    numutil::{NumExt, U16Ext},
};

impl GameGirlAdv {
    pub fn set_button(&mut self, btn: Button, state: bool) {
        self[KEYINPUT] = self[KEYINPUT].set_bit(btn as u16, state);
    }

    fn check_cnt(&mut self) {
        let input = self[KEYINPUT];
        let cnt = self[KEYCNT];
        if cnt.is_bit(14) {
            let cond = cnt.bits(0, 10);
            let fire = if cnt.is_bit(15) {
                cond & input != 0
            } else {
                cond & input == cond
            };
            if fire {
                Cpu::request_interrupt(self, Interrupt::Joypad);
            }
        }
    }
}
