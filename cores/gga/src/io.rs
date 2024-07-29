// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, self file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with self file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use ::common::{common::debugger::Severity, components::io::get_mmio_apply, numutil::NumExt, *};
use arm_cpu::{Cpu, Interrupt};
use components::io::{section, set_mmio_apply, FAILED_WRITE};

use crate::{
    addr::*,
    audio::{self, Apu},
    hw::dma::Dmas,
    GameGirlAdv,
};

impl GameGirlAdv {
    pub fn get_mmio<T: NumExt>(&self, addr: u32) -> T {
        get_mmio_apply(addr, |a| {
            // Memory / IRQ control
            io16!(a, IME, self.cpu.ime as u16);
            io16!(a, IE, self.cpu.ie as u16);
            io16!(a, IF, self.cpu.if_ as u16);
            io16!(a, WAITCNT, self.memory.waitcnt.into());

            // Timers + DMA
            for idx in 0..4 {
                io16!(
                    a,
                    TM0CNT_L + (idx * 4),
                    self.timers.time_read(idx.us(), self.scheduler.now())
                );
                io16!(
                    a,
                    TM0CNT_H + (idx * 4),
                    self.timers.control[idx.us()].into()
                );
                io16!(
                    a,
                    0xBA + (idx * 0xC),
                    Into::<u16>::into(self.dma.channels[idx.us()].ctrl)
                        & [0xF7E0, 0xF7E0, 0xF7E0, 0xFFE0][idx.us()]
                );
                // DMA length registers read 0
                io16!(a, 0xB8 + (idx * 0xC), 0);
            }

            // Graphics
            if matches!(a, 0x00..=0x54) {
                let out = self.ppu.regs.read(a);
                if out.2 != 5 {
                    return out;
                }
            }

            // Sound
            if matches!(a & !1, 0x60..=0x80 | 0x84 | 0x86 | 0x8A | 0x90..=0x9F) {
                let value = Apu::read_register_psg(&self.apu.cgb_chans, a.u16());
                return (value as u32, 0, 1);
            }
            io16!(a, SOUNDCNT_H, Into::<u16>::into(self.apu.cnt) & 0x770F);
            io16!(a, SOUNDBIAS_L, self.apu.bias.into());

            // Input
            io16!(a, KEYINPUT, self.keyinput());

            // Serial
            io16!(a, RCNT, self.serial.rcnt);

            // Known 0 registers
            if matches!(a, 0x136 | 0x142 | 0x15A | 0x206 | 0x20A | POSTFLG | 0x302) {
                return (0, a & 1, 2);
            }

            self.get_mmio_invalid(a)
        })
    }

    pub fn get_mmio_invalid(&self, a: u32) -> (u32, u32, u32) {
        self.c.debugger.log(
            "invalid-mmio-read-unknown",
            format!("Read from unknown IO register 0x{a:03X}, returning open bus"),
            Severity::Warning,
        );
        let value = self.invalid_read::<false>(0x400_0000);
        (value, a & 1, 2)
    }

    pub fn set_mmio<T: NumExt>(&mut self, addr: u32, value: T) {
        set_mmio_apply(addr, value, |a, v, m| {
            let s8 = section::<u8>(a, v, m);
            let s16 = section::<u16>(a, v, m);
            let s32 = section::<u32>(a, v, m);

            // Memory / IRQ control
            iow16!(a, IME, {
                self.cpu.ime = s16.with(0).is_bit(0);
                Cpu::check_if_interrupt(self);
            });
            iow16!(a, IE, {
                self.cpu.ie = s16.with(self.cpu.ie.u16()).u32();
                Cpu::check_if_interrupt(self);
            });
            iow16!(a, IF, {
                self.cpu.if_ &= !s16.raw().u32();
                Cpu::check_if_interrupt(self);
            });
            iow08!(a, HALTCNT, self.cpu.halt_on_irq());
            iow16!(a, WAITCNT, {
                let prev: u16 = self.memory.waitcnt.into();
                let new = s16.apply_io_ret(&mut self.memory.waitcnt);
                let value: u16 = new.into();

                // Only update things as needed
                if value.bits(0, 11) != prev.bits(0, 11) {
                    self.update_wait_times();
                }
                if value.bit(14) != prev.bit(14) {
                    self.memory.prefetch.active = false;
                    self.memory.prefetch.restart = true;
                    self.cpu.cache.invalidate_rom();
                } else if value.bits(2, 9) != prev.bits(2, 9) {
                    self.cpu.cache.invalidate_rom();
                }
            });

            // Graphics
            if matches!(a, 0x00..=0x54) {
                return self.ppu.regs.write(a, s8, s16);
            }

            // DMA audio
            iow16!(a, SOUNDCNT_H, s16.apply_io(&mut self.apu.cnt));
            iow16!(a, SOUNDBIAS_L, {
                s16.apply_io(&mut self.apu.bias); // TODO update sample rate
            });
            for i in 0..4 {
                iow08!(a, FIFO_A_L + i, self.apu.push_sample::<0>(s8.raw()));
                iow08!(a, FIFO_B_L + i, self.apu.push_sample::<1>(s8.raw()));
            }

            // CGB audio
            if matches!(a & !1, 0x60..=0x80 | 0x84 | 0x86 | 0x8A | 0x90..=0x9F) {
                let mut sched = audio::shed(&mut self.scheduler);
                Apu::write_register_psg(&mut self.apu.cgb_chans, a.u16(), s8.raw(), &mut sched);
                return (0, 1);
            }

            // Input
            iow16!(a, KEYCNT, {
                s16.apply_io(&mut self.memory.keycnt);
                self.check_keycnt();
            });

            // Timers + DMA
            for idx in 0..4 {
                iow16!(
                    a,
                    TM0CNT_L + (idx.u32() * 4),
                    s16.apply(&mut self.timers.reload[idx])
                );
                iow16!(a, TM0CNT_H + (idx.u32() * 4), {
                    self.timers.hi_write(&mut self.scheduler, idx, s16)
                });

                iow32!(
                    a,
                    0xB0 + (idx.u32() * 0xC),
                    s32.apply(&mut self.dma.channels[idx].sad)
                );
                iow32!(
                    a,
                    0xB4 + (idx.u32() * 0xC),
                    s32.apply(&mut self.dma.channels[idx].dad)
                );
                iow16!(
                    a,
                    0xB8 + (idx.u32() * 0xC),
                    s16.apply(&mut self.dma.channels[idx].count)
                );
                iow16!(
                    a,
                    0xBA + (idx.u32() * 0xC),
                    Dmas::ctrl_write(self, idx, s16)
                );
            }

            // Serial
            iow16!(a, SIOCNT, {
                if s16.raw() == 0x4003 {
                    Cpu::request_interrupt(self, Interrupt::Serial);
                }
            });
            iow16!(a, RCNT, s16.mask(0x41F0).apply_io(&mut self.serial.rcnt));

            // RO registers, or otherwise invalid
            if matches!(a, KEYINPUT
                        | 0x86
                        | 0x136
                        | 0x15A
                        | 0x206
                        | 0x300
                        | 0x302
                        | 0x56..=0x5E
                        | 0x8A..=0x8E
                        | 0xA8..=0xAF
                        | 0xE0..=0xFF
                        | 0x110..=0x12E
                        | 0x140..=0x15E
                        | 0x20A..=0x21E)
            {
                self.c.debugger.log(
                    "invalid-mmio-write-known",
                    format!(
                        "Write to known read-only IO register 0x{a:03X} (value {value:04X}), ignoring"
                    ),
                    Severity::Info,
                );
                return (a & 1, 2);
            }

            self.c.debugger.log(
                "invalid-mmio-write-unknown",
                format!("Write to unknown IO register 0x{a:03X} (value {value:04X}), ignoring"),
                Severity::Warning,
            );
            FAILED_WRITE
        })
    }
}
