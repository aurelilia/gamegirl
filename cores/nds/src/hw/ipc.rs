// Copyright (c) 2024 Leela Aurelia, git@elia.garden
//
// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL-2.0) or the
// GNU General Public License Version 3 (GPL-3).
// If a copy of these licenses was not distributed with this file, you can
// obtain them at https://mozilla.org/MPL/2.0/ and http://www.gnu.org/licenses/.

use std::{collections::VecDeque, default, mem};

use arm_cpu::Interrupt;
use arrayvec::ArrayVec;
use common::numutil::U32Ext;
use modular_bitfield::{bitfield, specifiers::*, BitfieldSpecifier};

use crate::CpuDevice;

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SyncRegister {
    data_in: B4,
    #[skip]
    __: B4,
    data_out: B4,
    #[skip]
    __: B1,
    send_irq: bool,
    irq_en: bool,
    #[skip]
    __: B1,
}

#[bitfield]
#[repr(u16)]
#[derive(Debug, Default, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ControlRegister {
    send_fifo_empty: bool,
    send_fifo_full: bool,
    send_fifo_empty_irq: bool,
    send_fifo_clear: bool,
    #[skip]
    __: B4,
    recv_fifo_empty: bool,
    recv_fifo_full: bool,
    recv_fifo_not_empty_irq: bool,
    #[skip]
    __: B3,
    error: bool,
    enable: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct FifoIrqs {
    sync: bool,
    send_empty: bool,
    recv_not_empty: bool,
}

impl Default for FifoIrqs {
    fn default() -> Self {
        Self {
            sync: false,
            send_empty: true,
            recv_not_empty: false,
        }
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CpuFifo {
    irqs: FifoIrqs,
    fifo_en: bool,
    error: bool,
    fifo: VecDeque<u32>,
    last: u32,
    sync_out: u8,
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct IpcFifo {
    state: IpcFifoState,
    cpu: CpuDevice<CpuFifo>,
}

impl IpcFifo {
    pub fn advance(&mut self, cpu: usize) -> Option<Interrupt> {
        if self.cpu[cpu].fifo_en {
            match mem::replace(&mut self.state, IpcFifoState::Clean) {
                IpcFifoState::Clean => None,

                IpcFifoState::Pushed(v) if self.cpu[cpu ^ 1].fifo.len() < 16 => {
                    self.cpu[cpu ^ 1].fifo.push_back(v);
                    let recv_irq =
                        self.cpu[cpu ^ 1].irqs.recv_not_empty && self.cpu[cpu ^ 1].fifo.len() == 1;
                    recv_irq.then_some(Interrupt::IpcRecvFifoNotEmpty)
                }
                IpcFifoState::Pushed(_) => {
                    self.cpu[cpu].error = true;
                    None
                }

                IpcFifoState::Popped => match self.cpu[cpu].fifo.pop_front() {
                    Some(v) => {
                        self.cpu[cpu].last = v;
                        let send_irq =
                            self.cpu[cpu ^ 1].irqs.send_empty && self.cpu[cpu].fifo.is_empty();
                        send_irq.then_some(Interrupt::IpcSendFifoEmpty)
                    }
                    None => {
                        self.cpu[cpu].error = true;
                        None
                    }
                },
            }
        } else {
            self.state = IpcFifoState::Clean;
            None
        }
    }

    pub fn receive(&mut self, cpu: usize) -> u32 {
        self.state = IpcFifoState::Popped;
        let result = self.cpu[cpu]
            .fifo
            .front()
            .copied()
            .unwrap_or(self.cpu[cpu].last);
        log::error!("CPU{cpu} read FIFO and got {result:X}");
        result
    }

    pub fn send_low(&mut self, cpu: usize, v: u16) {
        let pre = if let IpcFifoState::Pushed(v) = self.state {
            v
        } else {
            0
        };
        self.state = IpcFifoState::Pushed(pre.set_low(v));
    }

    pub fn send_high(&mut self, cpu: usize, v: u16) {
        let pre = if let IpcFifoState::Pushed(v) = self.state {
            v
        } else {
            0
        };
        self.state = IpcFifoState::Pushed(pre.set_high(v));
    }

    pub(crate) fn sync_read(&mut self, i: usize) -> u16 {
        let local = &self.cpu[i];
        let remote = &self.cpu[i ^ 1];
        SyncRegister::new()
            .with_data_in(remote.sync_out)
            .with_data_out(local.sync_out)
            .with_irq_en(local.irqs.sync)
            .into()
    }

    pub(crate) fn cnt_read(&mut self, i: usize) -> u16 {
        let local = &self.cpu[i];
        let remote = &self.cpu[i ^ 1];
        ControlRegister::new()
            .with_send_fifo_empty(remote.fifo.is_empty())
            .with_send_fifo_full(remote.fifo.len() == 16)
            .with_send_fifo_empty_irq(local.irqs.send_empty)
            .with_recv_fifo_empty(local.fifo.is_empty())
            .with_recv_fifo_full(local.fifo.len() == 16)
            .with_recv_fifo_not_empty_irq(local.irqs.recv_not_empty)
            .with_error(local.error)
            .with_enable(local.fifo_en)
            .into()
    }

    pub(crate) fn sync_write(&mut self, i: usize, value: u16) -> bool {
        let new = SyncRegister::from(value);
        let (a, b) = self.cpu.split_at_mut(1);
        let (local, remote) = if i == 0 {
            (&mut a[0], &mut b[0])
        } else {
            (&mut b[0], &mut a[0])
        };

        local.sync_out = new.data_out();
        local.irqs.sync = new.irq_en();
        new.send_irq()
    }

    pub(crate) fn cnt_write(&mut self, i: usize, value: u16) {
        let new = ControlRegister::from(value);
        let (a, b) = self.cpu.split_at_mut(1);
        let (local, remote) = if i == 0 {
            (&mut a[0], &mut b[0])
        } else {
            (&mut b[0], &mut a[0])
        };

        local.irqs.send_empty = new.send_fifo_empty_irq();
        local.irqs.recv_not_empty = new.recv_fifo_not_empty_irq();
        local.error &= !new.error();
        local.fifo_en = new.enable();

        if new.send_fifo_clear() {
            remote.fifo.clear();
            remote.last = 0;
        }
    }
}

#[derive(Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum IpcFifoState {
    #[default]
    Clean,
    Pushed(u32),
    Popped,
}
