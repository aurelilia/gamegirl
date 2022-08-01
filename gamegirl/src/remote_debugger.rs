// Unless otherwise noted, this file is released and thus subject to the
// terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
// "Incompatible With Secondary Licenses", as defined by the MPL2.
// If a copy of the MPL2 was not distributed with this file, you can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use common::{components::debugger::Breakpoint, numutil::NumExt};
use gdbstub::{
    arch::Arch,
    common::{Pid, Signal},
    conn::{Connection, ConnectionExt},
    stub::{
        run_blocking::{BlockingEventLoop, Event, WaitForStopReasonError},
        GdbStub, SingleThreadStopReason,
    },
    target::{
        ext::{
            base::{
                single_register_access::{SingleRegisterAccess, SingleRegisterAccessOps},
                singlethread::{
                    SingleThreadBase, SingleThreadResume, SingleThreadResumeOps,
                    SingleThreadSingleStep, SingleThreadSingleStepOps,
                },
                BaseOps,
            },
            breakpoints::{
                Breakpoints, BreakpointsOps, HwWatchpoint, HwWatchpointOps, SwBreakpoint,
                SwBreakpointOps, WatchKind,
            },
            exec_file::{ExecFile, ExecFileOps},
        },
        Target, TargetResult,
    },
};
use gdbstub_arch::arm::{reg::id::ArmCoreRegId, Armv4t};

use crate::{
    remote_debugger::DebuggerStatus::{Disconnected, Running, WaitingForConnection},
    System,
};

pub struct SyncSys(Arc<Mutex<System>>, bool, PathBuf);

impl SyncSys {
    fn lock(&mut self) -> MutexGuard<System> {
        self.0.lock().unwrap()
    }
}

impl Target for SyncSys {
    type Arch = Armv4t;
    type Error = String;

    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }

    fn support_breakpoints(&mut self) -> Option<BreakpointsOps<'_, Self>> {
        Some(self)
    }

    fn support_exec_file(&mut self) -> Option<ExecFileOps<'_, Self>> {
        Some(self)
    }
}

impl SingleThreadBase for SyncSys {
    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as Arch>::Registers,
    ) -> TargetResult<(), Self> {
        let gg = self.lock();
        let gg = gg.as_gga();

        for (i, reg) in regs.r.iter_mut().enumerate() {
            *reg = gg.cpu.reg(i.u32());
        }
        regs.sp = gg.cpu.sp();
        regs.lr = gg.cpu.lr();
        regs.pc = gg.cpu.pc();
        regs.cpsr = gg.cpu.cpsr;

        Ok(())
    }

    fn write_registers(
        &mut self,
        regs: &<Self::Arch as Arch>::Registers,
    ) -> TargetResult<(), Self> {
        let mut gg = self.lock();
        let mut gg = gg.gga_mut();

        for (i, reg) in regs.r.iter().enumerate() {
            gg.cpu.registers[i] = *reg;
        }
        gg.cpu.set_sp(regs.sp);
        gg.cpu.set_lr(regs.lr);
        if regs.pc != gg.cpu.pc() {
            // gg.set_pc(regs.pc); TODO
        }
        gg.cpu.cpsr = regs.cpsr;

        Ok(())
    }

    fn support_single_register_access(&mut self) -> Option<SingleRegisterAccessOps<'_, (), Self>> {
        Some(self)
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &mut [u8],
    ) -> TargetResult<(), Self> {
        let gg = self.lock();
        let gg = gg.as_gga();
        for (offs, data) in data.iter_mut().enumerate() {
            *data = gg.get_byte(start_addr + offs.u32());
        }
        Ok(())
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &[u8],
    ) -> TargetResult<(), Self> {
        let mut gg = self.lock();
        let gg = gg.gga_mut();
        for (offs, data) in data.iter().enumerate() {
            gg.set_byte(start_addr + offs.u32(), *data);
        }
        Ok(())
    }

    fn support_resume(&mut self) -> Option<SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }
}

impl SingleRegisterAccess<()> for SyncSys {
    fn read_register(
        &mut self,
        _tid: (),
        reg_id: <Self::Arch as Arch>::RegId,
        buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let gg = self.lock();
        let gg = gg.as_gga();

        let value = match reg_id {
            ArmCoreRegId::Gpr(id) => gg.cpu.reg(id.u32()),
            ArmCoreRegId::Sp => gg.cpu.sp(),
            ArmCoreRegId::Lr => gg.cpu.lr(),
            ArmCoreRegId::Pc => gg.cpu.pc(),
            ArmCoreRegId::Cpsr => gg.cpu.cpsr,
            _ => return Ok(0),
        };
        for (src, dst) in value.to_le_bytes().iter().zip(buf.iter_mut()) {
            *dst = *src;
        }

        Ok(4)
    }

    fn write_register(
        &mut self,
        _tid: (),
        reg_id: <Self::Arch as Arch>::RegId,
        val: &[u8],
    ) -> TargetResult<(), Self> {
        let value = u32::from_le_bytes([val[0], val[1], val[2], val[3]]);
        let mut gg = self.lock();
        let gg = gg.gga_mut();

        match reg_id {
            ArmCoreRegId::Gpr(id) => gg.cpu.registers[id.us()] = value,
            ArmCoreRegId::Sp => gg.cpu.set_sp(value),
            ArmCoreRegId::Lr => gg.cpu.set_lr(value),
            ArmCoreRegId::Pc => panic!(), // gg.set_pc(value), TODO
            ArmCoreRegId::Cpsr => gg.cpu.cpsr = value,
            _ => (),
        };
        Ok(())
    }
}

impl SingleThreadResume for SyncSys {
    fn resume(&mut self, _signal: Option<Signal>) -> Result<(), Self::Error> {
        self.0.lock().unwrap().gga_mut().options.running = true;
        Ok(())
    }

    fn support_single_step(&mut self) -> Option<SingleThreadSingleStepOps<'_, Self>> {
        Some(self)
    }
}

impl SingleThreadSingleStep for SyncSys {
    fn step(&mut self, _signal: Option<Signal>) -> Result<(), Self::Error> {
        self.0.lock().unwrap().gga_mut().advance();
        self.1 = true;
        Ok(())
    }
}

impl Breakpoints for SyncSys {
    fn support_sw_breakpoint(&mut self) -> Option<SwBreakpointOps<'_, Self>> {
        Some(self)
    }

    fn support_hw_watchpoint(&mut self) -> Option<HwWatchpointOps<'_, Self>> {
        Some(self)
    }
}

impl SwBreakpoint for SyncSys {
    fn add_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        _kind: <Self::Arch as Arch>::BreakpointKind,
    ) -> TargetResult<bool, Self> {
        let mut gg = self.lock();
        let gg = gg.gga_mut();
        gg.debugger.breakpoints.push(Breakpoint {
            value: Some(addr),
            value_text: addr.to_string(),
            pc: true,
            write: false,
        });
        Ok(true)
    }

    fn remove_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        _kind: <Self::Arch as Arch>::BreakpointKind,
    ) -> TargetResult<bool, Self> {
        let mut gg = self.lock();
        let gg = gg.gga_mut();
        let len = gg.debugger.breakpoints.len();
        gg.debugger
            .breakpoints
            .retain(|bp| !(bp.pc && bp.value == Some(addr)));
        Ok(len != gg.debugger.breakpoints.len())
    }
}

impl HwWatchpoint for SyncSys {
    fn add_hw_watchpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        _len: <Self::Arch as Arch>::Usize,
        kind: WatchKind,
    ) -> TargetResult<bool, Self> {
        if let WatchKind::Read | WatchKind::ReadWrite = kind {
            return Ok(false);
        }

        let mut gg = self.lock();
        let gg = gg.gga_mut();
        gg.debugger.breakpoints.push(Breakpoint {
            value: Some(addr),
            value_text: addr.to_string(),
            pc: false,
            write: true,
        });
        Ok(true)
    }

    fn remove_hw_watchpoint(
        &mut self,
        addr: <Self::Arch as Arch>::Usize,
        _len: <Self::Arch as Arch>::Usize,
        kind: WatchKind,
    ) -> TargetResult<bool, Self> {
        if let WatchKind::Read | WatchKind::ReadWrite = kind {
            return Ok(false);
        }

        let mut gg = self.lock();
        let gg = gg.gga_mut();
        let len = gg.debugger.breakpoints.len();
        gg.debugger
            .breakpoints
            .retain(|bp| !(bp.write && bp.value == Some(addr)));
        Ok(len != gg.debugger.breakpoints.len())
    }
}

impl ExecFile for SyncSys {
    fn get_exec_file(
        &self,
        _pid: Option<Pid>,
        offset: u64,
        length: usize,
        buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let path = self.2.canonicalize().unwrap();
        let path = path.as_os_str().as_bytes();
        let mut count = 0;
        for (src, dst) in path
            .iter()
            .skip(offset as usize)
            .zip(buf.iter_mut())
            .take(length)
        {
            *dst = *src;
            count += 1;
        }
        Ok(count)
    }
}

enum EventLoop {}

impl BlockingEventLoop for EventLoop {
    type Target = SyncSys;
    type Connection = TcpStream;
    type StopReason = SingleThreadStopReason<u32>;

    fn wait_for_stop_reason(
        target: &mut Self::Target,
        conn: &mut Self::Connection,
    ) -> Result<
        Event<Self::StopReason>,
        WaitForStopReasonError<
            <Self::Target as Target>::Error,
            <Self::Connection as Connection>::Error,
        >,
    > {
        let hit_bp = target.0.lock().unwrap().as_gga().options.running;
        match () {
            _ if hit_bp => Ok(Event::TargetStopped(SingleThreadStopReason::SwBreak(()))),
            _ if target.1 => {
                target.1 = false;
                Ok(Event::TargetStopped(SingleThreadStopReason::DoneStep))
            }
            _ => Ok(Event::IncomingData(
                conn.read().map_err(WaitForStopReasonError::Connection)?,
            )),
        }
    }

    fn on_interrupt(
        _target: &mut Self::Target,
    ) -> Result<Option<Self::StopReason>, <Self::Target as Target>::Error> {
        // TODO handle this in the GUI
        Ok(Some(SingleThreadStopReason::Signal(Signal::SIGINT)))
    }
}

#[derive(Copy, Clone)]
pub enum DebuggerStatus {
    NotActive,
    WaitingForConnection,
    Running(SocketAddr),
    Disconnected,
}

#[allow(clippy::needless_pass_by_value)]
pub fn init(sys: Arc<Mutex<System>>, rom_path: PathBuf, status: Arc<RwLock<DebuggerStatus>>) {
    {
        *status.write().unwrap() = WaitingForConnection;
    }
    let sock = TcpListener::bind("localhost:17633").unwrap();
    let (stream, address) = sock.accept().unwrap();
    {
        *status.write().unwrap() = Running(address);
    }

    let mut sys = SyncSys(sys, false, rom_path);
    let debugger = GdbStub::new(stream);
    match debugger.run_blocking::<EventLoop>(&mut sys) {
        Ok(_) => {
            *status.write().unwrap() = Disconnected;
        }
        Err(e) => {
            log::error!("gdbstub encountered an error: {}", e);
        }
    }
}
