use super::PortIoDevice;
use crate::{Error as HyperError, Result as HyperResult};
use x86::io;

pub struct PCIPassthrough {
    port_base: u16,
    current_address: u64,
}

impl PCIPassthrough {
    pub fn new(port_base: u16) -> Self {
        Self {
            port_base,
            current_address: 0,
        }
    }
}

impl PioOps for PCIPassthrough {
    fn port_range(&self) -> core::ops::Range<u16> {
        return self.port_base..self.port_base + 8;
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        match access_size {
            1 => Ok(unsafe { io::inb(port) } as u32),
            2 => Ok(unsafe { io::inw(port) } as u32),
            4 => Ok(unsafe { io::inl(port) }),
            _ => Err(HyperError::InvalidParam),
        }
    }

    fn write(&mut self, port: u16, access_size: u8, value: u32) -> HyperResult {
        match access_size {
            1 => Ok(unsafe { io::outb(port, value as u8) }),
            2 => Ok(unsafe { io::outw(port, value as u16) }),
            4 => Ok(unsafe { io::outl(port, value) }),
            _ => Err(HyperError::InvalidParam),
        }
    }
}
