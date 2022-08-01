// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::ops::Index;

use common::numutil::NumExt;
pub use inner::*;

use crate::PpuSystem;

pub type PpuMmio = [u16; 0x56 / 2];

impl<'t, S: PpuSystem> Index<u32> for PpuType<'t, S>
where
    [(); S::W * S::H]:,
{
    type Output = u16;

    fn index(&self, addr: u32) -> &Self::Output {
        assert!(addr < 0x56);
        assert_eq!(addr & 1, 0);
        &self.mmio[(addr >> 1).us()]
    }
}

#[cfg(not(feature = "threaded"))]
mod inner {
    use crate::{interface::PpuSystem, threading::PpuMmio, Ppu};

    pub type GgaPpu<S> = Ppu<S>;

    pub struct PpuType<'t, S: PpuSystem>
    where
        [(); S::W * S::H]:,
    {
        pub mmio: PpuMmio,
        pub ppu: &'t mut Ppu<S>,
    }

    pub fn new_ppu<S: PpuSystem>() -> GgaPpu<S>
    where
        [(); S::W * S::H]:,
    {
        Ppu::default()
    }
}

#[cfg(feature = "threaded")]
mod inner {
    use std::{
        sync::{mpsc, Arc, Mutex, MutexGuard},
        thread,
    };

    use common::Colour;

    use super::PpuMmio;
    use crate::{Ppu, PpuSystem};

    pub type PpuType<'t, S> = Threaded<'t, S>;

    pub struct Threaded<'t, S: PpuSystem>
    where
        [(); S::W * S::H]:,
    {
        pub(super) mmio: PpuMmio,
        pub ppu: MutexGuard<'t, Ppu<S>>,
    }

    pub fn new_ppu<S: PpuSystem>() -> GgaPpu<S>
    where
        [(); S::W * S::H]:,
    {
        let ppu = Arc::new(Mutex::new(Ppu::default()));
        let thread = RenderThread::new(&ppu);
        GgaPpu {
            ppu,
            thread,
            last_frame: None,
        }
    }

    pub struct GgaPpu<S: PpuSystem>
    where
        [(); S::W * S::H]:,
    {
        pub ppu: Arc<Mutex<Ppu<S>>>,
        pub thread: RenderThread,
        pub last_frame: Option<Vec<Colour>>,
    }

    #[cfg(feature = "serde")]
    impl<'de, S: PpuSystem> serde::Deserialize<'de> for GgaPpu<S>
    where
        [(); S::W * S::H]:,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let ppu = Arc::<Mutex<Ppu<S>>>::deserialize(deserializer)?;
            let thread = RenderThread::new(&ppu);
            Ok(GgaPpu {
                ppu,
                thread,
                last_frame: None,
            })
        }
    }

    #[cfg(feature = "serde")]
    impl<S: PpuSystem> serde::Serialize for GgaPpu<S>
    where
        [(); S::W * S::H]:,
    {
        fn serialize<SE>(&self, serializer: SE) -> Result<SE::Ok, SE::Error>
        where
            SE: serde::Serializer,
        {
            Arc::<Mutex<Ppu<S>>>::serialize(&self.ppu, serializer)
        }
    }

    pub struct RenderThread {
        mmio_sender: mpsc::Sender<PpuMmio>,
    }

    impl RenderThread {
        pub fn render(&self, mmio: PpuMmio) {
            self.mmio_sender.send(mmio).unwrap();
        }

        pub fn new<S: PpuSystem>(ppu: &Arc<Mutex<Ppu<S>>>) -> Self
        where
            [(); S::W * S::H]:,
        {
            let (tx, rx) = mpsc::channel();
            let ppu = Arc::clone(ppu);
            thread::spawn(move || loop {
                let mmio = match rx.recv() {
                    Ok(mmio) => mmio,
                    Err(_) => return,
                };
                let ppu_lock = ppu.lock().unwrap();
                Ppu::render_line(&mut Threaded {
                    mmio,
                    ppu: ppu_lock,
                });
            });

            Self { mmio_sender: tx }
        }
    }
}
