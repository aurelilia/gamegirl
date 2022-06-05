use gamegirl::system::cpu::DReg::{BC, DE, HL};
use gamegirl::system::cpu::Reg::A;
use std::ops::ControlFlow::{Break, Continue};

pub fn exec(subdir: &str) {
    crate::run_dir(&format!("mooneye/{subdir}"), |gg| {
        if gg.cpu.reg(A) == 0
            && gg.cpu.dreg(BC) == 0x0305
            && gg.cpu.dreg(DE) == 0x080D
            && gg.cpu.dreg(HL) == 0x1522
        {
            Break(true)
        } else {
            Continue(())
        }
    })
}
