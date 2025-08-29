#![no_std]
#![no_main]

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;
#[macro_use]
extern crate log;

#[unsafe(no_mangle)]
fn main() {
    println!("Test USB start...");

    let ls = arceos_usb::dev_list().unwrap();

    info!("USB Device List {}:", ls.len());
    for dev in ls {
        info!("  {dev}");
    }

    info!("Test USB done");
}
