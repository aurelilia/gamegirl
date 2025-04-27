// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use core::hash::Hasher;
use std::{boxed::Box, hash::DefaultHasher, string::String, vec::Vec};

use crate::{common::options::SystemConfig, components::storage::GameCart, Core};

pub type TestInspector<S> = fn(&mut Box<S>) -> TestStatus;

pub fn run_test<S: Core>(rom: &[u8], inspector: TestInspector<S>) {
    let rom = rom.into();
    let mut core = S::try_new(
        &mut Some(GameCart { rom, save: None }),
        &SystemConfig::default(),
    )
    .unwrap();
    for _ in 0..30 {
        core.advance_delta(1.0);
        let status = (inspector)(&mut core);
        match status {
            TestStatus::Running => continue,
            TestStatus::Success => return,
            TestStatus::Failed => panic!("Test failed!"),
            TestStatus::FailedAt(msg) => panic!("Test failed: {msg}!"),
        }
    }
    panic!("Test timed out!")
}

pub fn screen_hash<S: Core>(core: &mut Box<S>) -> u64 {
    let mut hasher = DefaultHasher::new();
    if let Some(frame) = core.c_mut().video_buffer.pop_recent() {
        hasher.write(
            &frame
                .into_iter()
                .flat_map(|t| t.into_iter())
                .collect::<Vec<_>>(),
        )
    }
    hasher.finish()
}

#[derive(PartialEq)]
pub enum TestStatus {
    Running,
    Success,
    Failed,
    FailedAt(String),
}
