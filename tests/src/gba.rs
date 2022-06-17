use std::ops::ControlFlow::{Break, Continue};

use crate::Status;

pub fn exec() {
    crate::run_dir("gba-tests", |gg| {
        let gg = gg.as_gga();
        if gg.cpu.sp() == 0x03008014 {
            let ones = gg.cpu.reg(10);
            let tens = gg.cpu.reg(9);
            let hundreds = gg.cpu.reg(8);
            let test = ones + (tens * 10) + (hundreds * 100);
            Break(Status::FailAt(test.to_string()))
        } else if gg.cpu.sp() == 0x03007F00 && gg.cpu.low[6] == 0x02000360 {
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}
