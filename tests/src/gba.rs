use std::ops::ControlFlow::{Break, Continue};

use crate::Status;

pub fn exec_gba_tests() {
    crate::run_dir("gba-tests", |gg| {
        let gg = gg.as_gga();
        if gg.cpu.sp() == 0x03008014 {
            let ones = gg.cpu.reg(10);
            let tens = gg.cpu.reg(9);
            let hundreds = gg.cpu.reg(8);
            let test = ones + (tens * 10) + (hundreds * 100);
            Break(Status::FailAt(test.to_string()))
        } else if gg.cpu.sp() == 0x03007F00 && gg.get_word(gg.cpu.pc) == 0xE92D4003 {
            // These tests set SP to this value; the current instruction is always 'b 0x0'
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}

pub fn exec_fuzzarm() {
    crate::run_dir("fuzzarm", |gg| {
        let gg = gg.as_gga();
        if gg.cpu.pc == 0x0800_00F4 {
            // These tests set PC to this value; the current instruction is always 'b 0x0'
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}
