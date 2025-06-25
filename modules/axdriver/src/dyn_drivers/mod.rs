use core::{error::Error, ops::Deref, ptr::NonNull};

use alloc::{boxed::Box, format};
use axerrno::AxError;
use axhal::mem::{PhysAddr, phys_to_virt};
use rdrive::{
    DeviceId, IrqConfig,
    register::{DriverRegister, DriverRegisterSlice},
};

mod cache;
mod intc;

#[cfg(feature = "block")]
pub mod blk;

/// Sets up the device driver subsystem.
pub fn setup(dtb: usize) {
    if dtb == 0 {
        warn!("Device tree base address is 0, skipping device driver setup.");
        return;
    }
    cache::setup_dma_api();
    let dtb_virt = phys_to_virt(dtb.into());
    if let Some(dtb) = NonNull::new(dtb_virt.as_mut_ptr()) {
        rdrive::init(rdrive::Platform::Fdt { addr: dtb }).unwrap();
        rdrive::register_append(&driver_registers());
        // rdrive::probe_pre_kernel().unwrap();
    }
}

#[allow(dead_code)]
/// maps a mmio physical address to a virtual address.
fn iomap(addr: PhysAddr, size: usize) -> Result<NonNull<u8>, Box<dyn Error>> {
    let virt = match axmm::iomap(addr, size) {
        Ok(val) => val,
        Err(AxError::AlreadyExists) => phys_to_virt(addr),
        Err(e) => {
            return Err(format!(
                "Failed to map MMIO region: {e:?} (addr: {addr:?}, size: {size:#x})"
            )
            .into());
        }
    };
    Ok(unsafe { NonNull::new_unchecked(virt.as_mut_ptr()) })
}

#[allow(dead_code)]
fn parse_fdt_irq(intc: DeviceId, irq: &[u32]) -> IrqConfig {
    let intc = rdrive::get::<rdif_intc::Intc>(intc).expect("No interrupt controller found");
    let intc = intc.lock().unwrap();
    let fdt_parse = intc.parse_dtb_fn().expect("No DTB parse function found");
    fdt_parse(irq).unwrap()
}

fn driver_registers() -> impl Deref<Target = [DriverRegister]> {
    unsafe extern "C" {
        fn __sdriver_register();
        fn __edriver_register();
    }

    unsafe {
        let len = __edriver_register as usize - __sdriver_register as usize;

        if len == 0 {
            return DriverRegisterSlice::empty();
        }

        let data = core::slice::from_raw_parts(__sdriver_register as _, len);

        DriverRegisterSlice::from_raw(data)
    }
}
