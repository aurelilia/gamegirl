// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

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
