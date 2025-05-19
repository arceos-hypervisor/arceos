pub use crate::arch::dispatch_irq;
pub use crate::arch::fetch_irq;

use crate::irq::IrqHandler;
use somehal::driver::intc::*;
use somehal::mem::cpu_idx_to_id;
/// The maximum number of IRQs.
pub const MAX_IRQ_COUNT: usize = 2048;

static mut IRQ_CHIP: u64 = 0;

pub(crate) unsafe fn init() {
    let chip = somehal::driver::get_dev!(Intc).unwrap();
    unsafe { IRQ_CHIP = (chip.descriptor.device_id).into() };
}

#[cfg(feature = "smp")]
pub(crate) unsafe fn init_secondary() {
    // enable_systick();
}

pub(crate) fn cpu_interface() -> &'static BoxCPU {
    somehal::irq::interface(unsafe { IRQ_CHIP }.into()).expect("no cpu interface")
}

fn modify_chip<F: Fn(&mut Hardware)>(f: F) {
    let mut g = somehal::driver::get_dev!(Intc)
        .unwrap()
        .spin_try_borrow_by(0.into())
        .unwrap();
    (f)(&mut g);
}

/// Enables or disables the given IRQ.
pub fn set_enable(irq: IrqConfig, enabled: bool, is_cpu_local: bool) {
    // ArceOS cpu_id is actually cpu_idx
    let cpu_idx = crate::cpu::this_cpu_id();

    trace!("cpu[{:?}] Irq set enable: {:?} {}", cpu_idx, irq, enabled);

    if is_cpu_local {
        if let CPUCapability::LocalIrq(cpu) = cpu_interface().capability() {
            cpu.irq_enable(irq.irq).unwrap();
            cpu.set_trigger(irq.irq, irq.trigger).unwrap();
            return;
        }
    }

    let cpu_hard_id = cpu_idx_to_id(cpu_idx.into());

    modify_chip(|c| {
        c.set_target_cpu(irq.irq, cpu_hard_id.raw().into()).unwrap();
        c.set_trigger(irq.irq, irq.trigger).unwrap();
        if enabled {
            c.irq_enable(irq.irq).unwrap();
        } else {
            c.irq_disable(irq.irq).unwrap();
        }
    });
}

/// Registers an IRQ handler for the given IRQ.
///
/// It also enables the IRQ if the registration succeeds. It returns `false` if
/// the registration failed.
pub fn register_handler(irq_config: IrqConfig, handler: IrqHandler) -> bool {
    trace!("register handler irq {:?}", irq_config);
    crate::irq::register_handler_common(irq_config, handler)
}
