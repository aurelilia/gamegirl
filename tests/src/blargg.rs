use std::ops::ControlFlow::{Break, Continue};

use crate::Status;

pub fn exec() {
    crate::run_dir::<true>("blargg", |gg| {
        let gg = gg.as_ggc();
        let serial = gg.debugger.serial_output.lock().unwrap();
        if serial.contains("Passed") {
            Break(Status::Success)
        } else if serial.contains("Failed") {
            Break(Status::FailAt(serial.lines().last().unwrap().to_string()))
        } else {
            Continue(())
        }
    })
}

pub fn exec_sound() {
    crate::run_dir::<true>("blargg_sound", |gg| {
        let gg = gg.as_ggc();
        if gg.mmu.read(0xA000) == 0 {
            Break(Status::Success)
        } else {
            Continue(())
        }
    })
}
