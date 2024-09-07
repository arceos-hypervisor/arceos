#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use std::thread;
use std::time::{Duration, Instant};

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("Hello, world!");

    let mut sec = 1;

    loop {
        println!("ArceOS sleep {} seconds ...", sec);
        let now = Instant::now();
        thread::sleep(Duration::from_secs(sec as _));
        let elapsed = now.elapsed();
        sec += 1;
        println!("ArceOS actual sleep {:?} seconds.", elapsed);
    }
}
