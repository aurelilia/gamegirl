use std::ops::ControlFlow::{Break, Continue};

pub fn exec() {
    crate::run_dir("blargg", |gg| {
        let dbg = gg.debugger.as_ref().unwrap().read().unwrap();
        if dbg.serial_output.contains("Passed") {
            Break(true)
        } else if dbg.serial_output.contains("Failed") {
            Break(false)
        } else {
            Continue(())
        }
    })
}
