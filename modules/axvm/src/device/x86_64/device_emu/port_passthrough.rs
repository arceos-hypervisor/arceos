use crate::{Error as HyperError, Result as HyperResult};
use hypercraft::PioOps;
use x86::io;

pub struct PortPassthrough {
    port_base: u16,
    count: u16,
}

impl PortPassthrough {
    pub fn new(port_base: u16, count: u16) -> Self {
        Self { port_base, count }
    }
}

impl PioOps for PortPassthrough {
    fn port_range(&self) -> core::ops::Range<u16> {
        return self.port_base..self.port_base + self.count;
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        match access_size {
            1 => Ok(unsafe { io::inb(port) } as u32),
            2 => Ok(unsafe { io::inw(port) } as u32),
            4 => Ok(unsafe { io::inl(port) }),
            _ => Err(HyperError::InvalidParam),
        }
    }

    fn write(&mut self, port: u16, access_size: u8, value: &[u8]) -> HyperResult {
        match access_size {
            1 => Ok(unsafe { io::outb(port, value as u8) }),
            2 => Ok(unsafe { io::outw(port, value as u16) }),
            4 => Ok(unsafe { io::outl(port, value) }),
            _ => Err(HyperError::InvalidParam),
        }
    }
}
