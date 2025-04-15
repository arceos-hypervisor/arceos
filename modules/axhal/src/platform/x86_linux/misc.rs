use x86::bits64::vmx;
use x86_64::instructions::port::PortWriteOnly;
use x86_64::registers::control::{Cr4, Cr4Flags};

use crate::cpu::{this_cpu_id, this_cpu_is_reserved};

/// Shutdown the whole system (in QEMU), including all CPUs.
///
/// See <https://wiki.osdev.org/Shutdown> for more information.
pub fn terminate() -> ! {
    info!("Shutting down...");

    if this_cpu_is_reserved() {
        // #[cfg(platform = "x86_64-qemu-q35")]
        unsafe { PortWriteOnly::new(0x604).write(0x2000u16) };
    } else {
        warn!("Instance {} CPU terminated", this_cpu_id());

        // Execute VMXOFF.
        unsafe {
            let _ = vmx::vmxoff();
            // Remove VMXE bit in CR4.
            Cr4::update(|cr4| cr4.remove(Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS));
        }
    }

    crate::arch::halt();
    warn!("It should shutdown!");
    loop {
        crate::arch::halt();
    }
}
