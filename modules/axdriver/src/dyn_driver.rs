extern crate alloc;

use axhal::mem::iomap;
use axhal::time::{busy_wait_until, wall_time};

use core::error::Error;
use core::time::Duration;

use axdriver_base::{BaseDriverOps, DeviceType};
use axdriver_base::{DevError, DevResult};
use axdriver_block::BlockDriverOps;

use rdif_base::ErrorBase;
use rdif_block::Interface;
use rdrive::{DeviceWeak, get_dev};

pub use rdrive::dev_list;

use rk3568_driver_block::{EMmcHost, EmmcDriver, Kernel, init_clk, set_impl};
use rk3568_driver_clk::ClkDriver;

use somehal::{
    driver::{Descriptor, HardwareKind, intc::Box, register::FdtInfo},
    module_driver,
};

pub struct KernelImpl;

impl Kernel for KernelImpl {
    fn sleep(us: u64) {
        let current_time = wall_time();
        let duration = Duration::from_micros(us);
        let deadline = current_time + duration;
        busy_wait_until(deadline);
    }
}

set_impl!(KernelImpl);

module_driver!(
    name: "eMMC",
    level: ProbeLevel::PreKernel,
    priority: ProbePriority::DEFAULT,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["rockchip,dwcmshc-sdhci"],
            on_probe: probe_mmc
        }
    ]
);

fn probe_mmc(info: FdtInfo<'_>, _dev: &Descriptor) -> Result<HardwareKind, Box<dyn Error>> {
    let mut reg = info
        .node
        .reg()
        .ok_or(alloc::format!("[{}] has no reg", info.node.name()))?;

    let mmc_reg = reg.next().unwrap();

    let mmc: core::ptr::NonNull<u8> = iomap(
        (mmc_reg.address as usize).into(),
        mmc_reg.size.unwrap_or(0x10000),
    )?;

    let _ = init_clk(0x7c);

    let mmc_address = mmc.as_ptr() as usize;

    debug!("mmc address: {:#x}", mmc_address);
    let mut emmc = EMmcHost::new(mmc_address);

    if emmc.init().is_ok() {
        log::info!("RK3568 eMMC: successfully initialized");
    } else {
        log::warn!("RK3568 eMMC: init failed");
    }

    Ok(HardwareKind::Block(Box::new(EmmcDriver(emmc))))
}

module_driver!(
    name: "Clock",
    level: ProbeLevel::PreKernel,
    priority: ProbePriority::CLK,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["rockchip,rk3568-cru"],
            on_probe: probe_clk
        }
    ]
);

fn probe_clk(info: FdtInfo<'_>, _dev: &Descriptor) -> Result<HardwareKind, Box<dyn Error>> {
    let mut reg = info
        .node
        .reg()
        .ok_or(alloc::format!("[{}] has no reg", info.node.name()))?;

    let cru_reg = reg.next().unwrap();

    let cru: core::ptr::NonNull<u8> = iomap(
        (cru_reg.address as usize).into(),
        cru_reg.size.unwrap_or(0x1000),
    )?;

    let cru_address = cru.as_ptr() as u64;

    debug!("cru address: {:#x}", cru_address);

    Ok(HardwareKind::Clk(Box::new(ClkDriver::new(cru_address))))
}

pub struct Block(DeviceWeak<Box<dyn Interface + 'static>>);

impl BaseDriverOps for Block {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
    fn device_name(&self) -> &str {
        self.0.descriptor.name
    }
}

impl BlockDriverOps for Block {
    fn num_blocks(&self) -> u64 {
        self.0.spin_try_borrow_by(0.into()).unwrap().num_blocks()
    }
    fn block_size(&self) -> usize {
        self.0.spin_try_borrow_by(0.into()).unwrap().block_size()
    }
    fn flush(&mut self) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .flush()
            .map_err(convert_error)
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .read_block(block_id, buf)
            .map_err(convert_error)
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .write_block(block_id, buf)
            .map_err(convert_error)
    }
}

impl From<DeviceWeak<Box<dyn Interface + 'static>>> for Block {
    fn from(base: DeviceWeak<Box<dyn Interface + 'static>>) -> Self {
        Self(base)
    }
}

fn convert_error(err: ErrorBase) -> DevError {
    match err {
        ErrorBase::Io => DevError::Io,
        ErrorBase::NoMem => DevError::NoMemory,
        ErrorBase::Again => DevError::Again,
        ErrorBase::Busy => DevError::ResourceBusy,
        ErrorBase::BadAddr(_) => DevError::BadState,
        ErrorBase::InvalidArg { .. } => DevError::InvalidParam,
    }
}
