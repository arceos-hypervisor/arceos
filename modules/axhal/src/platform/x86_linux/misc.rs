// use x86_64::instructions::port::PortWriteOnly;

/// Shutdown the whole system (in QEMU), including all CPUs.
///
/// See <https://wiki.osdev.org/Shutdown> for more information.
pub fn terminate() -> ! {
    info!("Shutting down...");
    axlog::ax_println!("\nBut now we need to find a way to return this core to Linux!!!\n");

    // #[cfg(platform = "x86_64-qemu-q35")]
    // unsafe {
    //     PortWriteOnly::new(0x604).write(0x2000u16)
    // };

    crate::arch::halt();
    warn!("It should shutdown!");
    loop {
        crate::arch::halt();
    }
}
