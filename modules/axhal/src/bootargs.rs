use core::ptr::{self, addr_of_mut};
use fdt_parser::Fdt;

const COMMAND_LINE_SIZE: usize = 2048;

static mut FDT_ADDR: usize = 0;
static mut BOOTARGS_LEN: usize = 0;
static mut BOOTARGS_CACHED: bool = false;
static mut BOOTARGS_BUFFER: [u8; COMMAND_LINE_SIZE] = [0; COMMAND_LINE_SIZE];

pub fn init_fdt(fdt: usize) {
    unsafe {
        FDT_ADDR = fdt;
    }
}

pub fn fdt() -> Option<Fdt<'static>> {
    let fdt_addr = unsafe { FDT_ADDR };
    info!("FDT address: {:#x}", fdt_addr);
    if fdt_addr == 0 {
        return None;
    }

    Fdt::from_ptr(core::ptr::NonNull::new(fdt_addr as *mut _)?).ok()
}

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

        BOOTARGS_LEN = bytes.len();
        BOOTARGS_CACHED = true;

        let slice = core::slice::from_raw_parts(buffer_ptr as *const u8, BOOTARGS_LEN);
        core::str::from_utf8(slice).ok()
    }
}
