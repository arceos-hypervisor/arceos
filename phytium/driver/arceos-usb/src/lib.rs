#![no_std]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use arceos_ext::iomap;
use axstd::os::arceos::modules::{axhal::{self, irq::register}, axlog::info};
use core::{ptr::NonNull, time::Duration};
use crab_usb::err::USBError;
use spin::{Mutex, Once};

pub use crab_usb::*;

const USB_3_HOST_MMIO_BASE: usize = 0x31a08000;
const USB_3_HOST_MMIO_SIZE: usize = 0x18000;
const USB_3_HOST_IRQ: usize = 0x30;

static USB_HOST: Once<Mutex<USBHost>> = Once::new();
static USB_IRQ_HANDLE: Once<EventHandler> = Once::new();

struct KernelImpl;
impl_trait! {
    impl Kernel for KernelImpl {
        fn sleep<'a>(duration: Duration) -> BoxFuture<'a, ()> {
            Box::pin(async move {
                axhal::time::busy_wait(duration);
            })
        }

        fn page_size() -> usize {
            0x1000
        }
    }
}
pub fn usb() -> &'static Mutex<USBHost> {
    USB_HOST.call_once(|| {
        let mmio_base = iomap(USB_3_HOST_MMIO_BASE.into(), USB_3_HOST_MMIO_SIZE).unwrap();

        let mut host = USBHost::new_xhci(mmio_base);

        let handle = host.event_handler();

        USB_IRQ_HANDLE.call_once(|| handle);

        register(USB_3_HOST_IRQ, irq_handler);

        spin_on::spin_on(async move {
            host.init().await.unwrap();
            Mutex::new(host)
        })
    })
}

fn irq_handler() {
    info!("USB IRQ");
    USB_IRQ_HANDLE.wait().handle_event();
}

pub fn dev_list() -> Result<Vec<DeviceInfo>, USBError> {
    spin_on::spin_on(async {
        let mut host = usb().lock();
        let ls = host.device_list().await?;
        Ok(ls.collect())
    })
}
