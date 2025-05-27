use core::error::Error;

extern crate alloc;

use arm_gic_driver::v2::Gic;
use axplat_dyn::{
    driver::{Descriptor, HardwareKind, intc::Box, register::FdtInfo},
    module_driver,
};

use crate::mem::iomap;

module_driver!(
    name: "GICv2",
    level: ProbeLevel::PreKernel,
    priority: ProbePriority::INTC,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["arm,cortex-a15-gic", "arm,gic-400"],
            on_probe: probe_gic
        },
    ] ,
);

fn probe_gic(info: FdtInfo<'_>, _dev: &Descriptor) -> Result<HardwareKind, Box<dyn Error>> {
    let mut reg = info
        .node
        .reg()
        .ok_or(alloc::format!("[{}] has no reg", info.node.name()))?;

    let gicd_reg = reg.next().unwrap();
    let gicc_reg = reg.next().unwrap();

    let gicd = iomap(
        (gicd_reg.address as usize).into(),
        gicd_reg.size.unwrap_or(0x1000),
    )
    .map_err(|e| alloc::format!("[{}] iomap gicd failed: {}", info.node.name(), e))?;
    let gicc = iomap(
        (gicc_reg.address as usize).into(),
        gicc_reg.size.unwrap_or(0x1000),
    )
    .map_err(|e| alloc::format!("[{}] iomap gicc failed: {}", info.node.name(), e))?;

    let gic = Gic::new(gicd, gicc);

    Ok(HardwareKind::Intc(Box::new(gic)))
}
