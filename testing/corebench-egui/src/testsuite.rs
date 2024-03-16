// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    fs,
    os::unix::ffi::OsStrExt,
    sync::{Arc, Mutex},
    thread,
};

use dynacore::{common::Core, NewCoreFn};
use walkdir::WalkDir;

pub type TestInspector = Box<dyn Fn(&Box<dyn Core>) -> TestStatus + Send + Sync>;

pub struct TestSuite {
    pub name: String,
    tests: Vec<Arc<Test>>,
    inspector: TestInspector,
}

impl TestSuite {
    pub fn run_on_core(self: Arc<Self>, loader: NewCoreFn) -> TestSuiteResult {
        let result = Arc::new(Mutex::new(
            self.tests
                .iter()
                .map(|t| TestResult {
                    test: Arc::clone(t),
                    result: TestStatus::Waiting,
                })
                .collect::<Vec<_>>(),
        ));
        let res = result.clone();

        thread::spawn(move || {
            'outer: for (i, test) in self.tests.iter().enumerate() {
                {
                    result.lock().unwrap()[i].result = TestStatus::Running
                }
                let mut core = loader(test.rom.clone());
                for _ in 0..30 {
                    core.advance_delta(1.0);
                    let status = (self.inspector)(&core);
                    if status != TestStatus::Running {
                        result.lock().unwrap()[i].result = status;
                        continue 'outer;
                    }
                }
                result.lock().unwrap()[i].result = TestStatus::FailedTimeout
            }
        });

        res
    }

    pub fn new(
        path: &str,
        inspector: impl Fn(&Box<dyn Core>) -> TestStatus + Send + Sync + 'static,
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
                    name: e.path().display().to_string(),
                }))
            });
        Self {
            name: path.to_string(),
            tests: tests.collect(),
            inspector: Box::new(inspector),
        }
    }
}

pub type TestSuiteResult = Arc<Mutex<Vec<TestResult>>>;

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
