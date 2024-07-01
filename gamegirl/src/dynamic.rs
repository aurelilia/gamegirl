// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::path::{Path, PathBuf};

use common::{misc::SystemConfig, Core};
use libloading::{Library, Symbol};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, INotifyWatcher, RecursiveMode, Watcher,
};

// We allow this here, since the library is only meant to be consumed by
// the testbench; which is compiled by the same version of the compiler
#[allow(improper_ctypes_definitions)]
pub type NewCoreFn = extern "C" fn(Vec<u8>) -> Box<dyn Core>;

// We allow this here, since the library is only meant to be consumed by
// the testbench; which is compiled by the same version of the compiler
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn new_core(cart: Vec<u8>) -> Box<dyn Core> {
    crate::load_cart(cart, None, &SystemConfig::default(), None, 0)
}

pub struct DynamicContext {
    loaded_cores: Vec<DynCore>,
    _watcher: INotifyWatcher,
}

impl DynamicContext {
    pub fn watch_dir(mut notify: impl FnMut(PathBuf) + Send + Sync + 'static) -> Self {
        let mut _watcher = notify::recommended_watcher(move |res| match res {
            Ok(notify::Event {
                kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
                mut paths,
                ..
            }) => notify(paths.pop().unwrap()),
            Ok(_) => (),
            Err(_) => panic!(),
        })
        .unwrap();
        _watcher
            .watch(Path::new("./dyn-cores"), RecursiveMode::Recursive)
            .unwrap();
        Self {
            loaded_cores: vec![],
            _watcher,
        }
    }

    pub fn load_core(&mut self, path: &Path) -> Result<usize, libloading::Error> {
        unsafe {
            let lib = Library::new(path)?;
            let fun: Symbol<NewCoreFn> = lib.get(b"new_core")?;
            self.loaded_cores.push(DynCore {
                loader: *fun,
                library: lib,
            });
        }
        Ok(self.loaded_cores.len() - 1)
    }

    pub fn get_core(&mut self, idx: usize) -> &DynCore {
        &self.loaded_cores[idx]
    }

    pub fn remove_core(&mut self, idx: usize) {
        let core = self.loaded_cores.remove(idx);
        core.library.close().unwrap();
    }
}

pub struct DynCore {
    library: Library,
    pub loader: NewCoreFn,
}
