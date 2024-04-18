mod apic_timer;
mod bundle;
mod debug_port;
mod dummy;
mod i8259_pic;
// mod pcip;
mod pit;
mod port_passthrough;
mod uart16550;

extern crate alloc;

use crate::Result as HyperResult;

pub use apic_timer::{ApicBaseMsrHandler, VirtLocalApic};
pub use bundle::Bundle;
pub use debug_port::DebugPort;
pub use dummy::Dummy;
use hypercraft::VirtMsrOps;
pub use i8259_pic::I8259Pic;
pub use port_passthrough::PortPassthrough;
pub use uart16550::{MultiplexConsoleBackend, Uart16550};

macro_rules! pmio_proxy_struct {
    ($port_begin:expr, $port_end:expr, $name:ident, $parent:ident, $reader:ident, $writer:ident) => {
        pub struct $name {
            parent: alloc::sync::Arc<spin::Mutex<$parent>>,
        }

        impl $crate::device::PioOps for $name {
            fn port_range(&self) -> core::ops::Range<u16> {
                ($port_begin)..(($port_end) + 1)
            }

            fn read(&mut self, port: u16, access_size: u8) -> crate::Result<u32> {
                self.parent.lock().$reader(port, access_size)
            }

            fn write(&mut self, port: u16, access_size: u8, value: u32) -> crate::Result {
                self.parent.lock().$writer(port, access_size, value)
            }
        }
    };
}

macro_rules! pmio_proxy_factory {
    ($fn:ident, $type:ident) => {
        pub fn $fn(some: &alloc::sync::Arc<spin::Mutex<Self>>) -> $type {
            $type {
                parent: some.clone(),
            }
        }
    };
}

macro_rules! msr_proxy_struct {
    ($msr_begin:expr, $msr_end:expr, $name:ident, $parent:ident, $reader:ident, $writer:ident) => {
        pub struct $name {
            parent: alloc::sync::Arc<spin::Mutex<$parent>>,
        }

        impl $crate::device::VirtMsrOps for $name {
            fn msr_range(&self) -> core::ops::Range<u32> {
                ($msr_begin)..(($msr_end) + 1)
            }

            fn read(&mut self, msr: u32) -> crate::Result<u64> {
                self.parent.lock().$reader(msr)
            }

            fn write(&mut self, msr: u32, value: u64) -> crate::Result {
                self.parent.lock().$writer(msr, value)
            }
        }
    };
}

macro_rules! msr_proxy_factory {
    ($fn:ident, $type:ident) => {
        pub fn $fn(some: &alloc::sync::Arc<spin::Mutex<Self>>) -> $type {
            $type {
                parent: some.clone(),
            }
        }
    };
}

pub(crate) use msr_proxy_factory;
pub(crate) use msr_proxy_struct;
pub(crate) use pmio_proxy_factory;
pub(crate) use pmio_proxy_struct;

pub struct MsrDummy {
    msr_range: core::ops::Range<u32>,
}

impl MsrDummy {
    pub fn new(msr: u32) -> Self {
        Self {
            msr_range: msr..msr + 1,
        }
    }

    pub fn new_range(range: core::ops::Range<u32>) -> Self {
        Self { msr_range: range }
    }
}

impl VirtMsrOps for MsrDummy {
    fn msr_range(&self) -> core::ops::Range<u32> {
        self.msr_range.clone()
    }

    fn read(&mut self, msr: u32) -> HyperResult<u64> {
        debug!("read from msr dummy {:#x}", msr);
        Ok(0)
    }

    fn write(&mut self, msr: u32, value: u64) -> HyperResult {
        debug!("write to msr dummy {:#x}, value: {:#x}", msr, value);
        Ok(())
    }
}
