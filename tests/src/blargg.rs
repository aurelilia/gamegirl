use std::ops::ControlFlow::{Break, Continue};

pub fn exec() {
    crate::run_dir("blargg", |gg| {
        let serial = gg.debugger.serial_output.lock().unwrap();
        if serial.contains("Passed") {
            Break(true)
        } else if serial.contains("Failed") {
            Break(false)
        } else {
            Continue(())
        }
    })
}

pub fn exec_sound() {
    crate::run_dir("blargg_sound", |gg| {
        if gg.mmu.read(0xA000) == 0 {
            Break(true)
        } else {
            Continue(())
        }
    })
}
