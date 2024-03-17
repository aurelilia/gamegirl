// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::testsuite::{TestStatus, TestSuite};

pub fn jsmolka() -> TestSuite {
    TestSuite::new("jsmolka", 10, |gg| {
        let regs = gg.get_registers();
        let hash = TestSuite::screen_hash(gg);

        if regs[13] == 0x03008014 {
            let ones = regs[10];
            let tens = regs[9];
            let hundreds = regs[8];
            let test = ones + (tens * 10) + (hundreds * 100);
            TestStatus::FailedAt(test.to_string())
        } else if [
            0x20974E0091874964,
            0x94F4D344B975EB0C,
            0x1A8992654BCDC4D8,
            0x63E68B6E5115B556,
        ]
        .contains(&hash)
        {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}

pub fn fuzzarm() -> TestSuite {
    TestSuite::new("fuzzarm", 20, |gg| {
        if TestSuite::screen_hash(gg) == 0xD5170621BA472629 {
            TestStatus::Success
        } else {
            TestStatus::Running
        }
    })
}
