#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

use core::time::Duration;

#[cfg(feature = "axstd")]
use axstd::os::arceos::api::time::ax_monotonic_time;
#[cfg(feature = "axstd")]
use axstd::os::arceos::modules;
#[cfg(feature = "axstd")]
use axstd::println;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        println!("Ticking!");

        println!("Current monotonic time: {:?}", ax_monotonic_time());

        loop {
            modules::axtask::sleep(Duration::from_millis(500));
            println!("Current monotonic time: {:?}", ax_monotonic_time());
        }
    }
}
