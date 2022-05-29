use crate::numutil::NumExt;
use crate::system::io::addr::DMA;
use crate::system::GameGirl;

#[derive(Default)]
pub struct Dma {
    time_left: i16,
}

impl Dma {
    pub fn step(gg: &mut GameGirl, t_cycles: usize) {
        if gg.mmu.dma.time_left <= 0 {
            return;
        }
        gg.mmu.dma.time_left -= t_cycles as i16;
        if gg.mmu.dma.time_left > 0 {
            return;
        }
        let mut src = gg.mmu[DMA].u16() * 0x100;
        for dest in 0xFE00..0xFEA0 {
            gg.mmu.write(dest, gg.mmu.read(src));
            src += 1;
        }
    }

    pub fn start(&mut self) {
        self.time_left = 648;
    }
}
