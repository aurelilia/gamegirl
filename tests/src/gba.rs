// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::ControlFlow::{Break, Continue};

use crate::Status;

pub fn exec_jsmolka() {
    crate::run_dir::<true, true>("jsmolka", |gg| {
        let gg = gg.as_gga();
        if gg.cpu.sp() == 0x03008014 {
            let ones = gg.cpu.reg(10);
            let tens = gg.cpu.reg(9);
            let hundreds = gg.cpu.reg(8);
            let test = ones + (tens * 10) + (hundreds * 100);
            Break(Status::FailAt(test.to_string()))
        } else {
            Continue(())
        }
    })
}

pub fn exec_fuzzarm() {
    crate::run_dir::<true, false>("fuzzarm", |gg| {
        let gg = gg.as_gga();
        if gg.cpu.pc() == 0x0800_00F4 {
            // These tests set PC to this value; the current instruction is always 'b 0x0'
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}

pub fn exec_ladystarbreeze() {
    crate::run_dir::<true, true>("ladystarbreeze", |_| Continue(()))
}

pub fn exec_destoer() {
    crate::run_dir::<true, true>("destoer", |_| Continue(()))
}
