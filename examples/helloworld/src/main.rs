#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("Hello, world!");
    let data_addr = 0xfd00_0008u32 as u32;
    // 直接读取该地址的值
    let data: u32 = unsafe { core::ptr::read_volatile(data_addr as *const u32) };
    println!("Data read from address 0xfd00_0000: {:#x}", data);
    // 直接写入一个新值
    let new_value: u32 = 0x1232_5678;
    println!("New value written to address 0xfd00_0000: {:#x}", new_value);
    unsafe {
        core::ptr::write_volatile(data_addr as *mut u32, new_value);
    }
    println!("OK");
}
