use super::PortIoDevice;
use libax::hv::{Result as HyperResult, Error as HyperError};

pub struct DebugPort {
    port: u16,
    range: u16
}

impl DebugPort {
    pub fn new(port: u16, range: u16) -> Self {
        Self { port, range }
    } 
}

impl PortIoDevice for DebugPort {
    fn port_range(&self) -> core::ops::Range<u16> {
        // self.port..self.port + 1
        self.port..self.port + self.range
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        // debug!("a byte read from debug port {:#x}", port);
        Ok(0)
    }

    fn write(&mut self, port: u16, access_size: u8, value: u32) -> HyperResult {
        // debug!("a byte written to debug port {:#x}: {:#4x}", port, value as u8);
        Ok(())
    }
}
