extern crate alloc;

use axhal::time::{busy_wait_until, wall_time};

use alloc::boxed::Box;
use core::error::Error;
use core::time::Duration;

use phytium_driver_block::{IoPad, Kernel, PAD_ADDRESS, SdCard, SdCardDriver, set_impl};

use rdrive::{Descriptor, HardwareKind, module_driver, register::FdtInfo};

use crate::drivers_dyn::iomap;

pub struct KernelImpl;

impl Kernel for KernelImpl {
    fn sleep(us: Duration) {
        let current_time = wall_time();
        let deadline = current_time + us;
        busy_wait_until(deadline);
    }
}

set_impl!(KernelImpl);

module_driver!(
    name: "Phytium SdCard",
    level: ProbeLevel::PostKernel,
    priority: ProbePriority::DEFAULT,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["phytium,mci"],
            on_probe: probe_sdcard
        }
    ],
);

fn probe_sdcard(info: FdtInfo<'_>, _dev: &Descriptor) -> Result<HardwareKind, Box<dyn Error>> {
    let mut reg = info
        .node
        .reg()
        .ok_or(alloc::format!("[{}] has no reg", info.node.name()))?;

    let mci_reg = reg.next().unwrap();

    let mci_reg_base = iomap(
        (mci_reg.address as usize).into(),
        mci_reg.size.unwrap_or(0x10000),
    )?;

    let iopad_reg_base = iomap((PAD_ADDRESS as usize).into(), 0x2000)?;

    let iopad = IoPad::new(iopad_reg_base);

    let sdcard = SdCard::new(mci_reg_base, iopad);

    Ok(HardwareKind::Block(Box::new(SdCardDriver::new(sdcard))))
}
