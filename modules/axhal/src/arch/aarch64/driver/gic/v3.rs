use core::error::Error;

extern crate alloc;

use arm_gic_driver::v3::Gic;
use somehal::{
    driver::{Descriptor, HardwareKind, intc::Box, register::Node},
    module_driver,
};

use crate::mem::iomap;

module_driver!(
    name: "GICv3",
    level: ProbeLevel::PreKernel,
    priority: ProbePriority::INTC,
    probe_kinds: &[
        ProbeKind::Fdt {
            compatibles: &["arm,gic-v3"],
            on_probe: probe_gic
        }
    ]
);

fn probe_gic(node: Node<'_>, _dev: &Descriptor) -> Result<HardwareKind, Box<dyn Error>> {
    let mut reg = node
        .reg()
        .ok_or(alloc::format!("[{}] has no reg", node.name()))?;

    let gicd_reg = reg.next().unwrap();
    let gicr_reg = reg.next().unwrap();

    let gicd = iomap(
        (gicd_reg.address as usize).into(),
        gicd_reg.size.unwrap_or(0x1000),
    )?;
    let gicr = iomap(
        (gicr_reg.address as usize).into(),
        gicr_reg.size.unwrap_or(0x1000),
    )?;

    let gic = Gic::new(gicd, gicr, Default::default());

    Ok(HardwareKind::Intc(Box::new(gic)))
}
