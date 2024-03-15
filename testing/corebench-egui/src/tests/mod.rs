// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::testsuite::{TestStatus, TestSuite};

pub const SUITES: &[(&str, fn() -> TestSuite)] = &[
    ("Blargg", blargg),
    ("Blargg-Sound", blargg_sound),
    ("Mooneye Acceptance", || mooneye("acceptance")),
];

pub fn blargg() -> TestSuite {
    TestSuite::new("blargg", |gg| {
        let serial = String::from_utf8_lossy(gg.get_serial());
        if serial.contains("Passed") {
            TestStatus::Success
        } else if serial.contains("Failed") {
            TestStatus::FailedAt(serial.lines().last().unwrap().to_string())
        } else {
            TestStatus::Running
        }
    })
}

pub fn blargg_sound() -> TestSuite {
    TestSuite::new("blargg_sound", |gg| {
        if gg.get_memory(0xA000) == 0 {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}

pub fn mooneye(subdir: &str) -> TestSuite {
    TestSuite::new(&format!("mooneye/{subdir}"), |gg| {
        let regs = gg.get_registers();
        if regs[0] == 0
            && regs[1] == 0x03
            && regs[2] == 0x05
            && regs[3] == 0x08
            && regs[4] == 0x0D
            && regs[6] == 0x15
            && regs[7] == 0x22
        {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}

pub fn _acid2() {
    // crate::run_dir::<true, true>("acid2", |_| Continue(()));
}
