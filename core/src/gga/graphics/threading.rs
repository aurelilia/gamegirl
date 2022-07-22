pub use inner::*;

#[cfg(not(feature = "threaded-ppu"))]
mod inner {
    use crate::{
        gga::{graphics::Ppu, GameGirlAdv},
        nds::Nds,
    };

    pub type GgaPpu = Ppu;
    pub type PpuType = GameGirlAdv;

    impl GameGirlAdv {
        #[inline]
        pub fn ppu(&mut self) -> &mut Ppu {
            &mut self.ppu
        }

        #[inline]
        pub fn ppu_nomut(&self) -> &Ppu {
            &self.ppu
        }
    }

    impl Nds {
        #[inline]
        pub fn ppu<const E: usize>(&mut self) -> MutexGuard<Ppu> {
            &self.ppus[E]
        }

        #[inline]
        pub fn ppu_nomut<const E: usize>(&self) -> MutexGuard<Ppu> {
            &mut self.ppus[E]
        }
    }

    pub fn new_ppu() -> GgaPpu {
        Ppu::default()
    }
}

#[cfg(feature = "threaded-ppu")]
mod inner {
    use std::{
        ops::Index,
        sync::{mpsc, Arc, Mutex, MutexGuard},
        thread,
    };

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::{
        gga::{graphics::Ppu, GameGirlAdv},
        nds::Nds,
        numutil::NumExt,
        Colour,
    };

    pub type PpuType<'t> = Threaded<'t>;
    type PpuMmio = [u16; 0x56 / 2];

    pub struct Threaded<'t> {
        mmio: PpuMmio,
        pub ppu: MutexGuard<'t, Ppu>,
    }

    impl<'t> Index<u32> for Threaded<'t> {
        type Output = u16;

        fn index(&self, addr: u32) -> &Self::Output {
            assert!(addr < 0x56);
            assert_eq!(addr & 1, 0);
            &self.mmio[(addr >> 1).us()]
        }
    }

    impl GameGirlAdv {
        #[inline]
        pub fn ppu(&mut self) -> MutexGuard<Ppu> {
            self.ppu.ppu.lock().unwrap()
        }

        #[inline]
        pub fn ppu_nomut(&self) -> MutexGuard<Ppu> {
            self.ppu.ppu.lock().unwrap()
        }
    }

    impl Nds {
        #[inline]
        pub fn ppu<const E: usize>(&mut self) -> MutexGuard<Ppu> {
            self.ppu.ppus[E].ppu.lock().unwrap()
        }

        #[inline]
        pub fn ppu_nomut<const E: usize>(&self) -> MutexGuard<Ppu> {
            self.ppu.ppus[E].ppu.lock().unwrap()
        }
    }

    pub fn new_ppu() -> GgaPpu {
        let ppu = Arc::new(Mutex::new(Ppu::default()));
        let thread = RenderThread::new(&ppu);
        GgaPpu {
            ppu,
            thread,
            last_frame: None,
        }
    }

    pub struct GgaPpu {
        pub ppu: Arc<Mutex<Ppu>>,
        pub thread: RenderThread,
        pub last_frame: Option<Vec<Colour>>,
    }

    impl<'de> Deserialize<'de> for GgaPpu {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let ppu = Arc::<Mutex<Ppu>>::deserialize(deserializer)?;
            let thread = RenderThread::new(&ppu);
            Ok(GgaPpu {
                ppu,
                thread,
                last_frame: None,
            })
        }
    }

    impl Serialize for GgaPpu {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Arc::<Mutex<Ppu>>::serialize(&self.ppu, serializer)
        }
    }

    pub struct RenderThread {
        mmio_sender: mpsc::Sender<PpuMmio>,
    }

    impl RenderThread {
        pub fn render(&self, mmio: PpuMmio) {
            self.mmio_sender.send(mmio).unwrap();
        }

        pub fn new(ppu: &Arc<Mutex<Ppu>>) -> Self {
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
