// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{marker::PhantomData, mem};

use arm_cpu::{Cpu, Interrupt};
pub use common::components::io::*;
use common::{io08, io16, io32, iow08, iow16, iow32, numutil::NumExt};

use crate::{addr::*, graphics::vram::*, hw::dma::Dmas, Nds, Nds7, Nds9, NdsCpu};

impl Nds {
    pub fn get_mmio_shared<DS: NdsCpu>(&mut self, a: u32) -> (u32, u32, u32) {
        // FIFO
        io16!(a, IPCSYNC, self.fifo.sync_read(DS::I));
        io16!(a, IPCFIFOCNT, self.fifo.cnt_read(DS::I));
        io32!(a, IPCFIFORECV_L, {
            let (value, intr) = self.fifo.receive(DS::I);
            self.maybe_irq_to_other(DS::I, intr);
            value
        });

        // Timers + DMA
        for idx in 0..4 {
            io16!(
                a,
                TM0CNT_L + (idx * 4),
                self.timers[DS::I].time_read(idx.us(), self.scheduler.now())
            );
            io16!(
                a,
                TM0CNT_H + (idx * 4),
                self.timers[DS::I].control[idx.us()].into()
            );
            io16!(
                a,
                0xBA + (idx * 0xC),
                self.dmas[DS::I].channels[idx.us()].ctrl.into()
            );
        }

        // GPU
        io16!(a, DISPSTAT, self.gpu.dispstat[DS::I].into());
        io16!(a, VCOUNT, self.gpu.vcount);

        // Input
        io16!(a, KEYCNT, self.input.cnt[DS::I].into());
        io16!(a, KEYINPUT, self.keyinput());

        // Misc
        io08!(a, POSTFLG, self.memory.postflg as u8);

        log::info!("Read from unknown IO register 0x{a:X}");
        FAILED_READ
    }

    pub fn set_mmio_shared<DS: NdsCpu>(&mut self, a: u32, v: u32, m: u32) -> (u32, u32) {
        let s8 = section::<u8>(a, v, m);
        let s16 = section::<u16>(a, v, m);
        let s32 = section::<u32>(a, v, m);

        // FIFO
        iow16!(a, IPCSYNC, {
            let send_irq = self.fifo.sync_write(DS::I, s16);
            if send_irq {
                self.send_irq(DS::I ^ 1, Interrupt::IpcSync);
            }
        });
        iow16!(a, IPCFIFOCNT, self.fifo.cnt_write(DS::I, s16));
        iow32!(a, IPCFIFOSEND_L, {
            let intr = self.fifo.send(DS::I, s32.with(0));
            self.maybe_irq_to_other(DS::I, intr);
        });

        // Timers + DMA
        for idx in 0..4 {
            iow16!(
                a,
                TM0CNT_H + (idx.u32() * 4),
                self.timers[DS::I].hi_write(DS::I == 1, &mut self.scheduler, idx, s16)
            );

            iow32!(
                a,
                0xB0 + (idx.u32() * 0xC),
                s32.apply(&mut self.dmas[DS::I].channels[idx].sad)
            );
            iow32!(
                a,
                0xB4 + (idx.u32() * 0xC),
                s32.apply(&mut self.dmas[DS::I].channels[idx].dad)
            );
            iow16!(
                a,
                0xB8 + (idx.u32() * 0xC),
                s16.apply(&mut self.dmas[DS::I].channels[idx].count)
            );

            if DS::I == 0 {
                iow16!(
                    a,
                    0xBA + (idx.u32() * 0xC),
                    Dmas::ctrl_write(&mut self.nds7(), idx, s16)
                );
            } else {
                iow16!(
                    a,
                    0xBA + (idx.u32() * 0xC),
                    Dmas::ctrl_write(&mut self.nds9(), idx, s16)
                );
            }
        }

        // Shared GPU stuff
        iow16!(a, DISPSTAT, {
            let disp: u16 = self.gpu.dispstat[DS::I].into();
            self.gpu.dispstat[DS::I] = ((disp & 0b111) | (s16.raw() & !0b1100_0111)).into();
        });

        // Input
        iow16!(a, KEYCNT, s16.apply_io(&mut self.input.cnt[DS::I]));

        // Misc
        iow08!(a, POSTFLG, self.memory.postflg = true);

        log::info!("Write to unknown IO register 0x{a:X}, value 0x{v:X}");
        FAILED_WRITE
    }
}

impl Nds7 {
    pub fn get_mmio<T: NumExt>(&mut self, addr: u32) -> T {
        get_mmio_apply(addr, |a| {
            // Memory / IRQ control
            io32!(a, IME, self.cpu7.ime as u32);
            io32!(a, IE_L, self.cpu7.ie);
            io32!(a, IF_L, self.cpu7.if_);
            io08!(a, VRAMSTAT, self.gpu.vram.vram_stat());
            io08!(a, WRAMSTAT, self.memory.wram_status as u8);

            // SPI
            io16!(a, SPICNT, self.spi.ctrl.into());
            io16!(a, SPIDATA, self.spi.data_out);

            self.get_mmio_shared::<Self>(a)
        })
    }

    pub fn set_mmio<T: NumExt>(&mut self, addr: u32, value: T) {
        set_mmio_apply(addr, value, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            let s16 = section::<u16>(a, v, m);
            let s32 = section::<u32>(a, v, m);

            // Memory / IRQ control
            iow32!(a, IME, {
                self.cpu7.ime = s32.with(0).is_bit(0);
                Cpu::check_if_interrupt(self);
            });
            iow32!(a, IE_L, {
                s32.apply(&mut self.cpu7.ie);
                Cpu::check_if_interrupt(self);
            });
            iow32!(a, IF_L, {
                s32.apply(&mut self.cpu7.if_);
                Cpu::check_if_interrupt(self);
            });
            iow08!(a, HALTCNT, self.cpu7.halt_on_irq());

            // SPI
            iow16!(a, SPICNT, s16.apply_io(&mut self.spi.ctrl));
            iow16!(a, SPIDATA, self.spi.data_write(s16.raw()));

            self.set_mmio_shared::<Self>(a, v, m)
        })
    }
}

impl Nds9 {
    pub fn get_mmio<T: NumExt>(&mut self, addr: u32) -> T {
        get_mmio_apply(addr, |a| {
            // Memory / IRQ control
            io32!(a, IME, self.cpu9.ime as u32);
            io32!(a, IE_L, self.cpu9.ie);
            io32!(a, IF_L, self.cpu9.if_);

            // Graphics
            if matches!(a, 0x00..=0x03 | 0x08..0x60) {
                return self.gpu.ppus[0].regs.read(a);
            }
            if matches!(a, 0x1000..=0x1003 | 0x1008..0x1060) {
                return self.gpu.ppus[1].regs.read(a & 0xFFF);
            }
            io16!(a, DISP3DCNT, self.gpu.gpu.cnt.into());
            io32!(a, DISPCAPCNT_L, self.gpu.capture.cnt.into());

            // RAM control
            io08!(a, VRAMCNT_A, self.gpu.vram.ctrls[A].into());
            io08!(a, VRAMCNT_B, self.gpu.vram.ctrls[B].into());
            io08!(a, VRAMCNT_C, self.gpu.vram.ctrls[C].into());
            io08!(a, VRAMCNT_D, self.gpu.vram.ctrls[D].into());
            io08!(a, VRAMCNT_E, self.gpu.vram.ctrls[E].into());
            io08!(a, VRAMCNT_F, self.gpu.vram.ctrls[F].into());
            io08!(a, VRAMCNT_G, self.gpu.vram.ctrls[G].into());
            io08!(a, WRAMCNT, self.memory.wram_status as u8);
            io08!(a, VRAMCNT_H, self.gpu.vram.ctrls[H].into());
            io08!(a, VRAMCNT_I, self.gpu.vram.ctrls[I].into());

            // DIV
            io16!(a, DIVCNT_L, self.div.ctrl.into());
            io32!(a, DIV_NUMER, self.div.numer as u32);
            io32!(a, DIV_NUMER_H, (self.div.numer >> 32) as u32);
            io32!(a, DIV_DENOM, self.div.denom as u32);
            io32!(a, DIV_DENOM_H, (self.div.denom >> 32) as u32);
            io32!(a, DIV_RESULT, self.div.result as u32);
            io32!(a, DIV_RESULT_H, (self.div.result >> 32) as u32);
            io32!(a, DIV_REM, self.div.rem as u32);
            io32!(a, DIV_REM_H, (self.div.rem >> 32) as u32);
            // SQRT
            io16!(a, SQRTCNT_L, self.sqrt.ctrl.into());
            io32!(a, SQRT_RESULT_L, self.sqrt.result);
            io32!(a, SQRT_INPUT, self.sqrt.input as u32);
            io32!(a, SQRT_INPUT, (self.sqrt.input >> 32) as u32);

            self.get_mmio_shared::<Self>(a)
        })
    }

    pub fn set_mmio<T: NumExt>(&mut self, addr: u32, value: T) {
        set_mmio_apply(addr, value, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            let s16 = section::<u16>(a, v, m);
            let s32 = section::<u32>(a, v, m);

            // Memory / IRQ control
            iow32!(a, IME, {
                self.cpu9.ime = s32.with(0).is_bit(0);
                Cpu::check_if_interrupt(self);
            });
            iow32!(a, IE_L, {
                s32.apply(&mut self.cpu9.ie);
                Cpu::check_if_interrupt(self);
            });
            iow32!(a, IF_L, {
                s32.apply(&mut self.cpu9.if_);
                Cpu::check_if_interrupt(self);
            });

            // Graphics
            if matches!(a, 0x00..=0x03 | 0x08..0x60) {
                return self.gpu.ppus[0].regs.write(a, s8, s16, s32);
            }
            if matches!(a, 0x1000..=0x1003 | 0x1008..0x1060) {
                return self.gpu.ppus[1].regs.write(a & 0xFFF, s8, s16, s32);
            }
            iow16!(a, DISP3DCNT, s16.apply_io(&mut self.gpu.gpu.cnt));
            iow32!(a, DISPCAPCNT_L, s32.apply_io(&mut self.gpu.capture.cnt));

            // RAM control
            let dsx: &mut Nds = &mut *self;
            for i in A..=G {
                iow08!(
                    a,
                    VRAMCNT_A + i.u32(),
                    dsx.gpu.vram.update_ctrl(
                        i,
                        s8.raw(),
                        &mut dsx.memory.pager7,
                        &mut dsx.memory.pager9
                    )
                );
            }
            iow08!(a, WRAMCNT, {
                dsx.memory.wram_status = unsafe { mem::transmute(s8.raw() & 3) };
                dsx.update_wram();
            });
            for i in H..=I {
                iow08!(
                    a,
                    VRAMCNT_B + i.u32(),
                    dsx.gpu.vram.update_ctrl(
                        i,
                        s8.raw(),
                        &mut dsx.memory.pager7,
                        &mut dsx.memory.pager9
                    )
                );
            }

            // Math
            // TODO React to writes.
            iow16!(a, DIVCNT_L, s16.apply_io(&mut self.div.ctrl));
            // SQRT
            iow16!(a, SQRTCNT_L, s16.apply_io(&mut self.sqrt.ctrl));

            self.set_mmio_shared::<Self>(a, v, m)
        })
    }
}
