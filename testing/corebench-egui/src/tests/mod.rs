// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

mod gb;
mod gba;

use gb::*;
use gba::*;

use crate::testsuite::TestSuite;

pub const SUITES: &[(&str, fn() -> TestSuite)] = &[
    ("Blargg", blargg),
    ("Blargg-Sound", blargg_sound),
    ("Mooneye Acceptance", || mooneye("acceptance")),
    ("Mooneye EmuOnly", || mooneye("emulator-only")),
    ("Mooneye Misc", || mooneye("misc")),
    ("GBMicrotest", || gbmicrotest()),
    ("{cgb,dmg}-acid2", || acid2()),
    ("jsmolka", || jsmolka()),
    ("fuzzarm", || fuzzarm()),
];
