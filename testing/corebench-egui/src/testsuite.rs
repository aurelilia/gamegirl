// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{
    fs,
    hash::{DefaultHasher, Hasher},
    os::unix::ffi::OsStrExt,
    sync::{Arc, Mutex},
    thread,
};

use gamegirl::{dynamic::NewCoreFn, Core};
use walkdir::WalkDir;

pub type TestInspector = Box<dyn Fn(&mut Box<dyn Core>) -> TestStatus + Send + Sync>;

pub struct TestSuite {
    pub name: String,
    tests: Vec<Arc<Test>>,
    inspector: TestInspector,
    time: usize,
}

impl TestSuite {
    pub fn run_on_core(self: Arc<Self>, loader: NewCoreFn) -> TestSuiteResult {
        let result = Arc::new(Mutex::new((
            self.tests
                .iter()
                .map(|t| TestResult {
                    test: Arc::clone(t),
                    result: TestStatus::Waiting,
                })
                .collect::<Vec<_>>(),
            0,
        )));
        let res = result.clone();

        thread::spawn(move || {
            'outer: for (i, test) in self.tests.iter().enumerate() {
                {
                    result.lock().unwrap().0[i].result = TestStatus::Running
                }
                let mut core = loader(test.rom.clone());
                for _ in 0..self.time {
                    core.advance_delta(1.0);
                    let status = (self.inspector)(&mut core);
                    if status != TestStatus::Running {
                        let mut res = result.lock().unwrap();
                        res.1 += (status == TestStatus::Success) as usize;
                        res.0[i].result = status;
                        continue 'outer;
                    }
                }
                result.lock().unwrap().0[i].result = TestStatus::FailedTimeout
            }
        });

        res
    }

    pub fn screen_hash(gg: &mut Box<dyn Core>) -> u64 {
        let mut hasher = DefaultHasher::new();
        if let Some(frame) = gg.c_mut().video_buffer.pop_recent() {
            hasher.write(
                &frame
                    .into_iter()
                    .flat_map(|t| t.into_iter())
                    .collect::<Vec<_>>(),
            )
        }
        hasher.finish()
    }

    pub fn new(
        path: &str,
        time: usize,
        inspector: impl Fn(&mut Box<dyn Core>) -> TestStatus + Send + Sync + 'static,
    ) -> Self {
        let tests = WalkDir::new(format!("testing/tests/{path}"))
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|e| e.as_bytes().starts_with(b"gb"))
                    && e.path()
                        .file_name()
                        .is_some_and(|e| !e.to_string_lossy().contains("disabled"))
            })
            .filter_map(|e| {
                Some(Arc::new(Test {
                    rom: fs::read(e.path()).ok()?,
                    name: e
                        .path()
                        .display()
                        .to_string()
                        .strip_prefix("testing/tests/")
                        .unwrap()
                        .to_string(),
                }))
            });
        Self {
            name: path.to_string(),
            tests: tests.collect(),
            inspector: Box::new(inspector),
            time,
        }
    }
}

pub type TestSuiteResult = Arc<Mutex<(Vec<TestResult>, usize)>>;

pub struct Test {
    pub rom: Vec<u8>,
    pub name: String,
}

pub struct TestResult {
    pub test: Arc<Test>,
    pub result: TestStatus,
}

#[derive(PartialEq)]
pub enum TestStatus {
    Waiting,
    Running,
    Success,
    Failed,
    FailedAt(String),
    FailedTimeout,
}
