//! Kernel boot arguments parsing and caching module.
//!
//! This module provides functionality to extract and cache kernel command line
//! arguments from the Flattened Device Tree (FDT). It implements a caching
//! mechanism to avoid expensive FDT parsing on repeated access.
//!
//! The boot arguments are typically passed from the bootloader and stored in
//! the `/chosen/bootargs` property of the device tree. This module provides
//! safe access to these arguments throughout the kernel's lifetime.

use core::ptr::{self, addr_of_mut};
use fdt_parser::Fdt;

/// Maximum size for the kernel command line arguments buffer
const COMMAND_LINE_SIZE: usize = 2048;

/// Global storage for the FDT (Flattened Device Tree) memory address
static mut FDT_ADDR: usize = 0;

/// Length of the cached bootargs string in bytes
static mut BOOTARGS_LEN: usize = 0;

/// Flag indicating whether bootargs have been parsed and cached
static mut BOOTARGS_CACHED: bool = false;

/// Static buffer to store the kernel command line arguments
static mut BOOTARGS_BUFFER: [u8; COMMAND_LINE_SIZE] = [0; COMMAND_LINE_SIZE];

/// Initialize the FDT subsystem with the device tree address
///
/// # Arguments
/// * `fdt` - Physical memory address where the FDT blob is located
pub fn init_fdt(fdt: usize) {
    unsafe {
        FDT_ADDR = fdt;
    }
}

/// Get a parsed FDT instance from the stored address
///
/// # Returns
/// * `Some(Fdt)` - Successfully parsed FDT if address is valid
/// * `None` - If FDT address is null or parsing fails
pub fn fdt() -> Option<Fdt<'static>> {
    let fdt_addr = unsafe { FDT_ADDR };
    info!("FDT address: {:#x}", fdt_addr);
    if fdt_addr == 0 {
        return None;
    }

    Fdt::from_ptr(core::ptr::NonNull::new(fdt_addr as *mut _)?).ok()
}

/// Retrieve the kernel boot arguments (command line) from the device tree
///
/// This function implements a caching mechanism to avoid repeatedly parsing
/// the FDT, which can be expensive. On first call, it extracts bootargs from
/// the device tree's /chosen node and caches them in a static buffer.
///
/// # Returns
/// * `Some(&str)` - The kernel command line as a UTF-8 string
/// * `None` - If FDT is unavailable, /chosen node missing, or bootargs invalid
pub fn bootargs() -> Option<&'static str> {
    unsafe {
        if BOOTARGS_CACHED {
            let slice = core::slice::from_raw_parts(
                addr_of_mut!(BOOTARGS_BUFFER) as *const u8,
                BOOTARGS_LEN,
            );
            return core::str::from_utf8(slice).ok();
        }

        let fdt = fdt()?;
        let chosen = fdt.chosen()?;
        let bootargs = chosen.bootargs()?;
        let bytes = bootargs.as_bytes();

        if bytes.len() > COMMAND_LINE_SIZE {
            return None;
        }

        let buffer_ptr = addr_of_mut!(BOOTARGS_BUFFER) as *mut u8;
        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer_ptr, bytes.len());

        // Update cache metadata
        BOOTARGS_LEN = bytes.len();
        BOOTARGS_CACHED = true;

        let slice = core::slice::from_raw_parts(buffer_ptr as *const u8, BOOTARGS_LEN);
        core::str::from_utf8(slice).ok()
    }
}
