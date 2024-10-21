#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "axstd")]
use std::os::arceos::api::config::SMP;
#[cfg(feature = "axstd")]
use std::os::arceos::api::task::{ax_set_current_affinity, AxCpuMask};
#[cfg(feature = "axstd")]
use std::os::arceos::modules::axhal::cpu::this_cpu_id;

const NUM_TIMES: usize = 5;

static FINISHED_TASKS: AtomicUsize = AtomicUsize::new(0);

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("Hello, world!");

    for i in 0..SMP {
        thread::spawn(move || {
            // Initialize cpu affinity here.
            #[cfg(feature = "axstd")]
            assert!(
                ax_set_current_affinity(AxCpuMask::one_shot(this_cpu_id())).is_ok(),
                "Initialize CPU affinity failed!"
            );

            println!(
                "Hello, task ({})! id = {:?} core {}",
                i,
                thread::current().id(),
                this_cpu_id()
            );
            for _t in 0..NUM_TIMES {
                println!(
                    "Hello, task ({})! id = {:?} core {}",
                    i,
                    thread::current().id(),
                    this_cpu_id()
                );
                thread::sleep(Duration::from_secs(1));
            }
            let _ = FINISHED_TASKS.fetch_add(1, Ordering::Relaxed);
        });
    }

    while FINISHED_TASKS.load(Ordering::Relaxed) < SMP {
        thread::yield_now();
    }

    println!("ArceOS SMP tests run OK!");
}
