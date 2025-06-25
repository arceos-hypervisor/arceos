#![no_std]
#![no_main]

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;

use std::os::arceos::modules::axhal;

#[unsafe(no_mangle)]
fn main() {
    println!("Hello, world!");
}
