#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;


use core::time::Duration;

use axstd::os::arceos::api;
use axstd::os::arceos::modules;
use axstd::println;

use api::time::ax_monotonic_time;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("Ticking!");

    println!("Current monotonic time: {:?}", ax_monotonic_time());

    loop {
        modules::axtask::sleep(Duration::from_millis(500));
        println!("Current monotonic time: {:?}", ax_monotonic_time());
    }
}
