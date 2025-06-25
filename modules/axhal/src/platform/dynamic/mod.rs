pub use axplat_dyn::driver;
#[allow(unused)]
pub use axplat_dyn::driver::intc::IrqConfig;

pub use axplat_dyn::fdt_ptr;

#[cfg(target_arch = "aarch64")]
mod aarch64_timer;
#[cfg(feature = "irq")]
pub(crate) mod irq;

use crate::bootargs::init_fdt;

unsafe extern "C" {
    fn rust_main(cpu_id: usize) -> !;
    #[cfg(feature = "smp")]
    fn rust_main_secondary(cpu_id: usize);
}

#[axplat_dyn::entry]
fn main(_cpu_id: usize, cpu_idx: usize) -> ! {
    // ArceOS soft cpu_id is cpu index.
    if cpu_idx == 0 {
        init_fdt(fdt_ptr() as usize);
        crate::cpu::init_primary(cpu_idx);
        unsafe { rust_main(cpu_idx) };
    } else {
        crate::cpu::init_secondary(cpu_idx);
        #[cfg(feature = "smp")]
        unsafe {
            rust_main_secondary(cpu_idx)
        }
    }
}

pub mod console {
    pub fn write_bytes(bytes: &[u8]) {
        axplat_dyn::console::write_bytes(bytes);
    }

    pub fn read_bytes(_bytes: &mut [u8]) -> usize {
        panic!("read_bytes is not implemented yet");
    }
}

pub mod time {
    pub fn current_ticks() -> u64 {
        axplat_dyn::systick::current_ticks()
    }

    /// Converts hardware ticks to nanoseconds.
    #[inline]
    pub fn ticks_to_nanos(ticks: u64) -> u64 {
        axplat_dyn::systick::ticks_to_nanos(ticks) as _
    }

    /// Converts nanoseconds to hardware ticks.
    #[inline]
    pub fn nanos_to_ticks(nanos: u64) -> u64 {
        axplat_dyn::systick::nanos_to_ticks(nanos as _)
    }

    /// Return epoch offset in nanoseconds (wall time offset to monotonic clock start).
    pub fn epochoffset_nanos() -> u64 {
        0
    }

    #[cfg(feature = "irq")]
    /// Set a one-shot timer.
    ///
    /// A timer interrupt will be triggered at the given deadline (in nanoseconds).
    pub fn set_oneshot_timer(deadline_ns: u64) {
        let ticks = current_ticks();
        let deadline = nanos_to_ticks(deadline_ns);
        let interval = if ticks < deadline {
            let interval = deadline - ticks;
            debug_assert!(interval <= u32::MAX as u64);
            interval
        } else {
            0
        };

        axplat_dyn::systick::get().set_timeval(interval as _);
        axplat_dyn::systick::get().set_irq_enable(true);
    }
}
pub mod misc {
    pub fn terminate() -> ! {
        axplat_dyn::power::terminate()
    }
}

/// Initializes the platform devices for the primary CPU.
///
/// For example, the interrupt controller and the timer.
pub fn platform_init() {
    unsafe {
        axplat_dyn::init();
        #[cfg(feature = "irq")]
        irq::init();

        axplat_dyn::driver::probe_all(true).unwrap();
    }
}

/// Initializes the platform devices for secondary CPUs.
#[cfg(feature = "smp")]
pub fn platform_init_secondary() {
    axplat_dyn::systick::set_enable(true);
    axplat_dyn::systick::get().set_irq_enable(true);
    axplat_dyn::systick::get().set_timeval(0);

    #[cfg(feature = "irq")]
    unsafe {
        irq::init_secondary();
    }
}
