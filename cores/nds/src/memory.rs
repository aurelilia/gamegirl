// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{mem, ptr};

use arm_cpu::{interface::RwType, Cpu, Interrupt};
use common::{
    common::debugger::Severity,
    components::{
        memory_mapper::{MemoryMappedSystem, MemoryMapper},
        thin_pager::{ThinPager, RO, RW},
    },
    numutil::{get_u64, hword, set_u64, word, ByteArrayExt, NumExt, U16Ext, U32Ext},
};

use super::{Nds7, Nds9};
use crate::{
    addr::*,
    cpu::cp15::TcmState,
    graphics::vram::*,
    hw::{dma::Dmas, timer::Timers},
    CpuDevice, Nds, NdsCpu,
};

pub const KB: usize = 1024;
pub const MB: usize = KB * KB;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum WramStatus {
    All9 = 0,
    First7 = 1,
    First9 = 2,
    All7 = 3,
}

/// Memory struct containing the NDS's memory regions along with page tables
/// and other auxiliary cached information relating to memory.
/// A lot is separated by the 2 CPUs.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Memory {
    pub psram: Box<[u8]>,
    wram: Box<[u8]>,
    pub wram_status: WramStatus,

    pub bios7: Box<[u8]>,
    pub bios9: Box<[u8]>,

    wram7: Box<[u8]>,
    pub(crate) tcm: [Box<[u8]>; 2],

    wait_word: CpuDevice<[u16; 32]>,
    wait_other: CpuDevice<[u16; 32]>,
    pub(crate) pager7: ThinPager,
    pub(crate) pager9: ThinPager,
}

impl Nds {
    /// Initialize page tables and wait times.
    pub fn init_memory(&mut self) {
        // Init 7
        let p7 = &mut self.memory.pager7;
        p7.init(0xFFF_FFFF);
        p7.map(&self.memory.bios7, 0x000_0000..0x100_0000, RO);
        p7.map(&self.memory.psram, 0x200_0000..0x300_0000, RW);
        p7.map(&self.memory.wram7, 0x380_0000..0x400_0000, RW);

        // Init 9
        let p9 = &mut self.memory.pager9;
        p9.init(0xFFF_FFFF);
        p9.map(&self.memory.tcm[1], 0x000_0000..0x200_0000, RW);
        p9.map(&self.memory.psram, 0x200_0000..0x300_0000, RW);

        // Init V/WRAM
        self.gpu.vram.init_mappings(p7, p9);
        self.update_wram();
    }

    fn update_wram(&mut self) {
        self.memory.pager7.evict(0x300_0000..0x380_0000);
        self.memory.pager9.evict(0x300_0000..0x400_0000);
        match self.memory.wram_status {
            WramStatus::All7 => {
                self.memory
                    .pager7
                    .map(&self.memory.wram, 0x300_0000..0x380_0000, RW)
            }
            WramStatus::First9 => {
                self.memory
                    .pager9
                    .map(&self.memory.wram[..(16 * KB)], 0x300_0000..0x400_0000, RW);
                self.memory
                    .pager7
                    .map(&self.memory.wram[(16 * KB)..], 0x300_0000..0x380_0000, RW);
            }
            WramStatus::First7 => {
                self.memory
                    .pager7
                    .map(&self.memory.wram[..(16 * KB)], 0x300_0000..0x380_0000, RW);
                self.memory
                    .pager9
                    .map(&self.memory.wram[(16 * KB)..], 0x300_0000..0x400_0000, RW);
            }
            WramStatus::All9 => {
                self.memory
                    .pager9
                    .map(&self.memory.wram, 0x300_0000..0x400_0000, RW);
                // When the shared WRAM isn't mapped, the ARM7 WRAM takes over
                self.memory
                    .pager7
                    .map(&self.memory.wram7, 0x300_0000..0x380_0000, RW)
            }
        }
    }

    #[allow(invalid_reference_casting)]
    pub fn try_get_mmio_shared<DS: NdsCpu>(ds: &mut DS, addr: u32) -> u16 {
        match addr & 0xFF_FFFF {
            // Interrupt control
            IME => ds.cpur().ime as u16,
            IE_L => ds.cpur().ie.low(),
            IE_H => ds.cpur().ie.high(),
            IF_L => ds.cpur().if_.low(),
            IF_H => ds.cpur().if_.high(),

            // FIFO
            IPCSYNC => ds.fifo.sync_read(DS::I),
            IPCFIFOCNT => ds.fifo.cnt_read(DS::I),
            IPCFIFORECV_L => ds.fifo.receive(DS::I).low(),
            IPCFIFORECV_H => ds.fifo.receive(DS::I).high(),

            // Timers
            TM0CNT_L => ds.timers[DS::I].time_read(0, ds.scheduler.now()),
            TM1CNT_L => ds.timers[DS::I].time_read(1, ds.scheduler.now()),
            TM2CNT_L => ds.timers[DS::I].time_read(2, ds.scheduler.now()),
            TM3CNT_L => ds.timers[DS::I].time_read(3, ds.scheduler.now()),
            TM0CNT_H => ds.timers[DS::I].control[0].into(),
            TM1CNT_H => ds.timers[DS::I].control[1].into(),
            TM2CNT_H => ds.timers[DS::I].control[2].into(),
            TM3CNT_H => ds.timers[DS::I].control[3].into(),

            // GPU
            DISPSTAT => ds.gpu.dispstat[DS::I].into(),
            VCOUNT => ds.gpu.vcount,

            // Input
            KEYCNT => ds.input.cnt[DS::I].into(),
            KEYINPUT => ds.keyinput(),

            // DMA
            0xBA => Into::<u16>::into(ds.dmas[DS::I].channels[0].ctrl),
            0xC6 => Into::<u16>::into(ds.dmas[DS::I].channels[1].ctrl),
            0xD2 => Into::<u16>::into(ds.dmas[DS::I].channels[2].ctrl),
            0xDE => Into::<u16>::into(ds.dmas[DS::I].channels[3].ctrl),

            // Timers
            _ => {
                ds.c.debugger.log(
                    "unknown-io-read",
                    format!("Read from unknown IO register {addr:08X}"),
                    Severity::Warning,
                );
                0
            }
        }
    }

    pub fn try_set_mmio_shared<DS: NdsCpu>(dsx: &mut DS, addr: u32, value: u16) {
        let ds = dsx.deref_mut();
        match addr & 0xFF_FFFF {
            // Interrupts
            IME => {
                dsx.cpu().ime = value.is_bit(0);
                Cpu::check_if_interrupt(dsx);
            }
            IE_L => {
                dsx.cpu().ie = word(value, dsx.cpu().ie.high());
                Cpu::check_if_interrupt(dsx);
            }
            IE_H => {
                dsx.cpu().ie = word(dsx.cpu().ie.low(), value);
                Cpu::check_if_interrupt(dsx);
            }
            IF_L => dsx.cpu().if_ &= (!value).u32() | 0xFFFF_0000,
            IF_H => dsx.cpu().if_ &= ((!value).u32() << 16) | 0x0000_FFFF,

            // FIFO
            IPCSYNC => {
                let send_irq = ds.fifo.sync_write(DS::I, value);
                if send_irq {
                    ds.send_irq(DS::I ^ 1, Interrupt::IpcSync);
                }
            }
            IPCFIFOCNT => ds.fifo.cnt_write(DS::I, value),
            IPCFIFOSEND_L => ds.fifo.send_low(DS::I, value),
            IPCFIFOSEND_H => ds.fifo.send_high(DS::I, value),

            // Timers
            TM0CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 0, value),
            TM1CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 1, value),
            TM2CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 2, value),
            TM3CNT_H => ds.timers[DS::I].hi_write(DS::I == 1, &mut ds.scheduler, 3, value),

            // DMAs
            0xBA => Dmas::ctrl_write(dsx, 0, value),
            0xC6 => Dmas::ctrl_write(dsx, 1, value),
            0xD2 => Dmas::ctrl_write(dsx, 2, value),
            0xDE => Dmas::ctrl_write(dsx, 3, value),

            // Shared GPU stuff
            DISPSTAT => {
                let disp: u16 = ds.gpu.dispstat[DS::I].into();
                ds.gpu.dispstat[DS::I] = ((disp & 0b111) | (value & !0b1100_0111)).into();
            }

            // Input
            KEYCNT => ds.input.cnt[DS::I] = value.into(),

            _ => ds.c.debugger.log(
                "unknown-io-write",
                format!("Write to unknown IO register {addr:08X}"),
                Severity::Warning,
            ),
        }
    }

    fn advance_fifo(&mut self, cpu: usize) {
        if let Some(intr) = self.fifo.advance(cpu) {
            self.send_irq(cpu ^ 1, intr);
        }
    }

    pub fn send_irq(&mut self, cpu: usize, irq: Interrupt) {
        if cpu == 0 {
            Cpu::request_interrupt(&mut self.nds7(), irq);
        } else {
            Cpu::request_interrupt(&mut self.nds9(), irq);
        }
    }
}

impl Nds7 {
    pub fn get<T: RwType>(&mut self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr > 0xFFF_FFFF {
            return T::from_u32(0);
        }
        if let Some(read) = self.memory.pager7.read(addr) {
            return read;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // MMIO
            0x04 => {
                let v = match T::WIDTH {
                    1 if addr.is_bit(0) => T::from_u8(self.get_mmio(addr).high()),
                    1 => T::from_u8(self.get_mmio(addr).low()),
                    2 => T::from_u16(self.get_mmio(addr)),
                    4 => T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2))),
                    _ => unreachable!(),
                };
                self.advance_fifo(Self::I);
                v
            }

            _ => T::from_u8(0),
        }
    }

    fn get_mmio(&mut self, addr: u32) -> u16 {
        let addr = addr & !1;
        match addr & 0xFF_FFFF {
            VRAMSTAT => hword(self.gpu.vram.vram_stat(), self.memory.wram_status as u8),
            EXTKEYIN => self.keyinput_ext(),
            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        if addr > 0xFFF_FFFF {
            return;
        }
        if let Some(write) = self.memory.pager7.write(addr) {
            *write = value;
            return;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // MMIO
            0x04 => {
                match T::WIDTH {
                    1 if addr.is_bit(0) => {
                        let l = self.get_mmio(addr).low();
                        self.set_mmio(addr, hword(l, value.u8()))
                    }
                    1 => {
                        let h = self.get_mmio(addr).high();
                        self.set_mmio(addr, hword(value.u8(), h));
                    }
                    2 => self.set_mmio(addr, value.u16()),
                    4 => {
                        self.set_mmio(addr, value.u16());
                        self.set_mmio(addr + 2, value.u32().high());
                    }
                    _ => unreachable!(),
                }
                self.advance_fifo(Self::I);
            }

            _ => {
                log::error!("Invalid write: {addr:X}");
            }
        }
    }

    fn set_mmio(&mut self, addr: u32, value: u16) {
        let addr = addr & !1;
        Nds::try_set_mmio_shared(self, addr, value);
    }
}

impl Nds9 {
    pub fn get<T: RwType>(&mut self, addr_unaligned: u32) -> T {
        let addr = addr_unaligned & !(T::WIDTH - 1);
        if addr <= 0xFFF_FFFF {
            for tcm in 0..2 {
                if self.cp15.tcm_state[tcm] == TcmState::Rw
                    && self.cp15.tcm_range[tcm].contains(&addr)
                {
                    return self.memory.tcm[tcm]
                        .get_wrap(addr.us() - self.cp15.tcm_range[tcm].start.us());
                }
            }

            if let Some(read) = self.memory.pager9.read(addr) {
                return read;
            }
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // Basic
            0xFF if addr >= 0xFFFF_0000 => self.memory.bios9.get_exact(a & 0xFFFF),

            // PPU
            // TODO verify the bit is right
            0x05 => self.gpu.ppus[a.bit(12)].palette.get_wrap(a),
            0x07 => self.gpu.ppus[a.bit(12)].oam.get_wrap(a),

            // MMIO
            0x04 => {
                let v = match T::WIDTH {
                    1 if addr.is_bit(0) => T::from_u8(self.get_mmio(addr).high()),
                    1 => T::from_u8(self.get_mmio(addr).low()),
                    2 => T::from_u16(self.get_mmio(addr)),
                    4 => T::from_u32(word(self.get_mmio(addr), self.get_mmio(addr + 2))),
                    _ => unreachable!(),
                };
                self.advance_fifo(Self::I);
                v
            }

            _ => {
                log::error!("Invalid read: {addr:X}");
                T::from_u32(0)
            }
        }
    }

    fn get_mmio(&mut self, addr: u32) -> u16 {
        let addr = addr & 0xFF_FFFE;
        match addr {
            // PPUs
            DISPCNT_L | DISPCNT_H | 0x08..0x60
                if let Some(val) = self.gpu.ppus[0].regs.read_mmio(addr) =>
            {
                val
            }
            0x1000 | 0x1002 | 0x1008..0x1060
                if let Some(val) = self.gpu.ppus[1].regs.read_mmio(addr & 0xFF) =>
            {
                val
            }

            // Graphics
            DISP3DCNT => self.gpu.gpu.cnt.into(),
            DISPCAPCNT_L => u32::from(self.gpu.capture.cnt).low(),
            DISPCAPCNT_H => u32::from(self.gpu.capture.cnt).high(),

            // RAM control
            VRAMCNT_A => hword(self.gpu.vram.ctrls[A].into(), self.gpu.vram.ctrls[B].into()),
            VRAMCNT_C => hword(self.gpu.vram.ctrls[C].into(), self.gpu.vram.ctrls[D].into()),
            VRAMCNT_E => hword(self.gpu.vram.ctrls[E].into(), self.gpu.vram.ctrls[F].into()),
            VRAMCNT_G => hword(self.gpu.vram.ctrls[G].into(), self.memory.wram_status as u8),
            VRAMCNT_H => hword(self.gpu.vram.ctrls[H].into(), self.gpu.vram.ctrls[I].into()),

            // Math
            DIVCNT_L => self.div.ctrl.into(),
            DIVCNT_H => 0,
            DIV_NUMER..DIV_DENOM => get_u64(self.div.numer, addr & 6),
            DIV_DENOM..DIV_RESULT => get_u64(self.div.denom, addr & 6),
            DIV_RESULT..DIV_REM => get_u64(self.div.result, addr & 6),
            DIV_REM..SQRTCNT_L => get_u64(self.div.rem, addr & 6),
            SQRTCNT_L => self.sqrt.ctrl.into(),
            SQRTCNT_H => 0,
            SQRT_RESULT_L => self.sqrt.result.low(),
            SQRT_RESULT_H => self.sqrt.result.high(),
            SQRT_INPUT..0x2C0 => get_u64(self.sqrt.input, addr & 6),

            _ => Nds::try_get_mmio_shared(self, addr),
        }
    }

    pub fn set<T: RwType>(&mut self, addr: u32, value: T) {
        for tcm in 0..2 {
            if self.cp15.tcm_state[tcm] != TcmState::None
                && self.cp15.tcm_range[tcm].contains(&addr)
            {
                return self.memory.tcm[tcm]
                    .set_wrap(addr.us() - self.cp15.tcm_range[tcm].start.us(), value);
            }
        }
        if addr > 0xFFF_FFFF {
            return;
        }
        if let Some(write) = self.memory.pager9.write(addr) {
            *write = value;
            return;
        }

        let region = addr >> 24;
        let a = addr.us();
        match region {
            // PPU
            // TODO verify the bit is right
            0x05 => self.gpu.ppus[a.bit(12)].palette.set_wrap(a, value),
            0x07 => self.gpu.ppus[a.bit(12)].oam.set_wrap(a, value),

            // MMIO
            0x04 => {
                match T::WIDTH {
                    1 if addr.is_bit(0) => {
                        let l = self.get_mmio(addr).low();
                        self.set_mmio_hword(addr, hword(l, value.u8()))
                    }
                    1 => {
                        let h = self.get_mmio(addr).high();
                        self.set_mmio_hword(addr, hword(value.u8(), h));
                    }
                    2 => self.set_mmio_hword(addr, value.u16()),
                    4 => self.set_mmio_word(addr, value.u32()),
                    _ => unreachable!(),
                }
                self.advance_fifo(Self::I);
            }

            _ => {
                log::error!("Invalid write: {addr:X}");
            }
        }
    }

    fn set_mmio_hword(&mut self, addr: u32, value: u16) {
        let addr = addr & 0xFF_FFFE;
        let dsx: &mut Nds = &mut *self;
        match addr {
            // PPUs
            // TODO handle byte writes right
            DISPCNT_L | DISPCNT_H | 0x08..0x60 => self.gpu.ppus[0].regs.write_mmio(addr, value),
            0x1000 | 0x1002 | 0x1008..0x1060 => {
                self.gpu.ppus[1].regs.write_mmio(addr & 0xFF, value)
            }

            // Graphics
            DISP3DCNT => self.gpu.gpu.cnt = value.into(),
            DISPCAPCNT_L => {
                self.gpu.capture.cnt = word(value, u32::from(self.gpu.capture.cnt).high()).into()
            }
            DISPCAPCNT_H => {
                self.gpu.capture.cnt = word(u32::from(self.gpu.capture.cnt).low(), value).into()
            }

            // RAM control
            VRAMCNT_A => {
                dsx.gpu.vram.update_ctrl(
                    A,
                    value.low(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
                dsx.gpu.vram.update_ctrl(
                    B,
                    value.high(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
            }
            VRAMCNT_C => {
                dsx.gpu.vram.update_ctrl(
                    C,
                    value.low(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
                dsx.gpu.vram.update_ctrl(
                    D,
                    value.high(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
            }
            VRAMCNT_E => {
                dsx.gpu.vram.update_ctrl(
                    E,
                    value.low(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
                dsx.gpu.vram.update_ctrl(
                    F,
                    value.high(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
            }
            VRAMCNT_G => {
                dsx.gpu.vram.update_ctrl(
                    G,
                    value.low(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
                dsx.memory.wram_status = unsafe { mem::transmute(value.high() & 3) };
                dsx.update_wram();
            }
            VRAMCNT_H => {
                dsx.gpu.vram.update_ctrl(
                    H,
                    value.low(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
                dsx.gpu.vram.update_ctrl(
                    I,
                    value.high(),
                    &mut dsx.memory.pager7,
                    &mut dsx.memory.pager9,
                );
            }

            // Math
            // TODO React to writes.
            DIVCNT_L => self.div.ctrl = value.into(),
            DIV_NUMER..DIV_DENOM => self.div.numer = set_u64(self.div.numer, addr & 6, value),
            DIV_DENOM..DIV_RESULT => self.div.denom = set_u64(self.div.denom, addr & 6, value),
            SQRTCNT_L => self.sqrt.ctrl = value.into(),
            SQRT_INPUT..0x2C0 => self.sqrt.input = set_u64(self.sqrt.input, addr & 6, value),

            _ => Nds::try_set_mmio_shared(self, addr, value),
        }
    }

    fn set_mmio_word(&mut self, addr: u32, value: u32) {
        let addr = addr & 0x1FFE;
        match addr {
            _ => {
                self.set_mmio_hword(addr, value.u16());
                self.set_mmio_hword(addr + 2, value.u32().high());
            }
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            psram: Box::new([0; 4 * MB]),
            wram: Box::new([0; 32 * KB]),
            wram_status: WramStatus::All9,
            bios7: Box::new([]),
            bios9: Box::new([]),

            wram7: Box::new([0; 64 * KB]),
            tcm: [Box::new([0; 16 * KB]), Box::new([0; 32 * KB])],

            wait_word: [[0; 32]; 2],
            wait_other: [[0; 32]; 2],
            pager7: ThinPager::default(),
            pager9: ThinPager::default(),
        }
    }
}

unsafe impl Send for Memory {}
