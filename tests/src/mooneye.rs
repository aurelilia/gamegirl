use core::ggc::cpu::{
    DReg::{BC, DE, HL},
    Reg::A,
};
use std::ops::ControlFlow::{Break, Continue};

use crate::Status;

pub fn exec(subdir: &str) {
    crate::run_dir(&format!("mooneye/{subdir}"), |gg| {
        let gg = gg.as_ggc();
        if gg.cpu.reg(A) == 0
            && gg.cpu.dreg(BC) == 0x0305
            && gg.cpu.dreg(DE) == 0x080D
            && gg.cpu.dreg(HL) == 0x1522
        {
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}
