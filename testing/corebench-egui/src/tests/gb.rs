// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use gamegirl::common::common::debugger::Width;

use crate::testsuite::{TestStatus, TestSuite};

pub fn blargg() -> TestSuite {
    TestSuite::new("blargg", 15, |gg| {
        let screen = TestSuite::screen_hash(gg);
        let serial = &gg.c().debugger.serial_output;

        // 2 tests don't properly write to serial
        if serial.contains("Passed") || [0xC595AEECEFF2C241, 0x115124ABCB508E19].contains(&screen) {
            TestStatus::Success
        } else if serial.contains("Failed") {
            TestStatus::FailedAt(serial.lines().last().unwrap().to_string())
        } else {
            TestStatus::Running
        }
    })
}

pub fn blargg_sound() -> TestSuite {
    TestSuite::new("blargg_sound", 30, |gg| {
        if gg.get_memory(0xA000, Width::Byte) == 0 {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}

pub fn mooneye(subdir: &str) -> TestSuite {
    TestSuite::new(&format!("mooneye/{subdir}"), 10, |gg| {
        let regs = gg.get_registers();
        if regs[1] == 0x03
            && regs[2] == 0x05
            && regs[3] == 0x08
            && regs[4] == 0x0D
            && regs[6] == 0x15
            && regs[7] == 0x22
        {
            TestStatus::Success
        } else if regs[1] == 0x42 && regs[2] == 0x42 && regs[3] == 0x42 {
            TestStatus::Failed
        } else {
            TestStatus::Running
        }
    })
}

pub fn gbmicrotest() -> TestSuite {
    TestSuite::new("c-sp/gbmicrotest", 5, |gg| {
        if gg.get_memory(0xFF82, Width::Byte) == 0x01 {
            TestStatus::Success
        } else if gg.get_memory(0xFF82, Width::Byte) == 0xFF {
            TestStatus::Failed
        } else {
            TestStatus::Running
        }
    })
}

pub fn acid2() -> TestSuite {
    TestSuite::new("acid2", 5, |gg| {
        let hash = TestSuite::screen_hash(gg);
        if [0xB60125B2D40BCBD9, 0xD0F0889F5971A43E].contains(&hash) {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}
