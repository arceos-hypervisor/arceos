//! Interrupt management.

use core::{sync::atomic::AtomicBool, task::Waker};

#[cfg(feature = "ipi")]
pub use axconfig::devices::IPI_IRQ;
use axcpu::trap::{IRQ, register_trap_handler};
#[cfg(feature = "ipi")]
pub use axplat::irq::{IpiTarget, send_ipi};
pub use axplat::irq::{handle, register, set_enable, unregister};
use axpoll::PollSet;

static POLL_TABLE: [PollSet; 2048] = [const { PollSet::new() }; 2048];
static REGISTERED: [AtomicBool; 2048] = [const { AtomicBool::new(false) }; 2048];

fn poll_handler(irq: usize) {
    unsafe extern "C" {
        fn handle_console_irq(irq: u32) -> u64;
    }
    unsafe { handle_console_irq(irq as u32) };
    POLL_TABLE[irq].wake();
}

/// Registers a waker for a IRQ interrupt.
pub fn register_irq_waker(irq: u32, waker: &Waker) {
    POLL_TABLE[irq as usize].register(waker);

    if !REGISTERED[irq as usize]
        .compare_exchange(
            false,
            true,
            core::sync::atomic::Ordering::SeqCst,
            core::sync::atomic::Ordering::SeqCst,
        )
        .is_ok()
    {
        return;
    }

    axplat::irq::register(irq as usize, poll_handler);
}

/// IRQ handler.
///
/// # Warn
///
/// Make sure called in an interrupt context or hypervisor VM exit handler.
#[register_trap_handler(IRQ)]
pub fn irq_handler(vector: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    handle(vector);
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}
