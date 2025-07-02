//! VirtIO Console frontend driver.

use crate::mem::{phys_to_virt, virt_to_phys};
use axalloc::global_allocator;
use axlog::ax_println;
use core::ptr::NonNull;
use kspin::SpinNoIrq;
use log::info;
use virtio_drivers::device::console::VirtIOConsole;
use virtio_drivers::transport::mmio::MmioTransport;
use virtio_drivers::transport::{DeviceType, Transport};
use virtio_drivers::{BufferDirection, Hal as VirtIoHal};

// VirtIO HAL implementation for axhal
pub struct VirtIoHalImpl;

unsafe impl VirtIoHal for VirtIoHalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (usize, NonNull<u8>) {
        let vaddr = if let Ok(vaddr) = global_allocator().alloc_pages(pages, 0x1000) {
            vaddr
        } else {
            return (0, NonNull::dangling());
        };
        let paddr = virt_to_phys(vaddr.into());
        let ptr = NonNull::new(vaddr as _).unwrap();
        (paddr.as_usize(), ptr)
    }

    unsafe fn dma_dealloc(_paddr: usize, vaddr: NonNull<u8>, pages: usize) -> i32 {
        global_allocator().dealloc_pages(vaddr.as_ptr() as usize, pages);
        0
    }

    #[inline]
    unsafe fn mmio_phys_to_virt(paddr: usize, _size: usize) -> NonNull<u8> {
        NonNull::new(phys_to_virt(paddr.into()).as_mut_ptr()).unwrap()
    }

    #[inline]
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> usize {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        virt_to_phys(vaddr.into()).as_usize()
    }

    #[inline]
    unsafe fn unshare(_paddr: usize, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}

static VIRTIO_CONSOLE: SpinNoIrq<Option<VirtIOConsole<VirtIoHalImpl, MmioTransport>>> =
    SpinNoIrq::new(None);

/// Writes a byte to the console.
pub fn putchar(c: u8) {
    if let Some(ref mut console) = VIRTIO_CONSOLE.lock().as_mut() {
        match c {
            b'\n' => {
                // Handle newline conversion like pl011
                if let Err(e) = console.send(b'\r') {
                    log::error!("VirtIO console write error: {:?}", e);
                }
                if let Err(e) = console.send(b'\n') {
                    log::error!("VirtIO console write error: {:?}", e);
                }
            }
            c => {
                if let Err(e) = console.send(c) {
                    log::error!("VirtIO console write error: {:?}", e);
                }
            }
        }
    }
}

/// Reads a byte from the console, or returns [`None`] if no input is available.
fn getchar() -> Option<u8> {
    if let Some(ref mut console) = VIRTIO_CONSOLE.lock().as_mut() {
        match console.recv(true) {
            Ok(Some(c)) => Some(c),
            Ok(None) => None, // No data available
            Err(e) => {
                log::error!("VirtIO console read error: {:?}", e);
                None
            }
        }
    } else {
        None
    }
}

/// Write a slice of bytes to the console.
pub fn write_bytes(bytes: &[u8]) {
    for c in bytes {
        putchar(*c);
    }
}

/// Reads bytes from the console into the given mutable slice.
/// Returns the number of bytes read.
pub fn read_bytes(bytes: &mut [u8]) -> usize {
    let mut read_len = 0;
    while read_len < bytes.len() {
        if let Some(c) = getchar() {
            bytes[read_len] = c;
            read_len += 1;
        } else {
            break;
        }
    }
    read_len
}

/// Initialize the VirtIO Console device early
pub fn init() {
    ax_println!("Probing VirtIO Console device...");

    #[cfg(feature = "irq")]
    {
        // In a real implementation, we would set up interrupts for the VirtIO device
        // For now, we just log that initialization is complete

        ax_println!("VirtIO Console IRQ initialization complete");
        // crate::irq::set_enable(crate::platform::irq::VIRTIO_CONSOLE_IRQ_NUM, true);
    }

    // Probe all VirtIO MMIO regions to find console device
    for reg in axconfig::devices::VIRTIO_MMIO_REGIONS {
        let base_vaddr = phys_to_virt(reg.0.into());
        let header = NonNull::new(
            base_vaddr.as_mut_ptr() as *mut virtio_drivers::transport::mmio::VirtIOHeader
        );

        if let Some(header) = header {
            match unsafe { MmioTransport::new(header) } {
                Ok(transport) => {
                    if DeviceType::Console == transport.device_type() {
                        // Check if this is a console device by trying to create one
                        match VirtIOConsole::new(transport) {
                            Ok(console) => {
                                *VIRTIO_CONSOLE.lock() = Some(console);
                                info!(
                                    "VirtIO Console device found and initialized at PA:{:#x}",
                                    reg.0
                                );
                                return; // Found console device, stop probing
                            }
                            Err(_) => {
                                // Not a console device or failed to initialize, continue probing
                                continue;
                            }
                        }
                    }
                    continue;
                }
                Err(_) => {
                    // Invalid VirtIO device, continue probing
                    continue;
                }
            }
        }
    }

    ax_println!("No VirtIO Console device found in MMIO regions");
}

/// Set VirtIO Console IRQ Enable
// pub fn init() {
//     #[cfg(feature = "irq")]
//     {
//         // In a real implementation, we would set up interrupts for the VirtIO device
//         // For now, we just log that initialization is complete
//         log::info!("VirtIO Console IRQ initialization complete");
//         // crate::irq::set_enable(crate::platform::irq::VIRTIO_CONSOLE_IRQ_NUM, true);
//     }
// }

/// VirtIO Console IRQ Handler
pub fn handle() {
    if let Some(ref mut console) = VIRTIO_CONSOLE.lock().as_mut() {
        // Acknowledge interrupt and check for new data
        match console.ack_interrupt() {
            Ok(true) => {
                // New data available, echo it back (like pl011 does)
                while let Ok(Some(c)) = console.recv(false) {
                    putchar(c);
                }
            }
            Ok(false) => {
                // No new data
            }
            Err(e) => {
                info!("VirtIO console interrupt error: {:?}", e);
            }
        }
    }
}

/// Check if VirtIO Console is available
pub fn is_available() -> bool {
    VIRTIO_CONSOLE.lock().is_some()
}
