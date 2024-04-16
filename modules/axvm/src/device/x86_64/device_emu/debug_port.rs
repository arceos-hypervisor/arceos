use crate::device::PioOps;
use crate::Result as HyperResult;

pub struct DebugPort {
    port: u16,
}

impl DebugPort {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
}

impl PioOps for DebugPort {
    fn port_range(&self) -> core::ops::Range<u16> {
        self.port..self.port + 1
    }

    fn read(&mut self, _port: u16, _access_size: u8) -> HyperResult<u32> {
        // debug!("a byte read from debug port {:#x}", port);
        Ok(0)
    }

    fn write(&mut self, _port: u16, _access_size: u8, _value: &[u8]) -> HyperResult {
        // debug!("a byte written to debug port {:#x}: {:#4x}", port, value as u8);
        Ok(())
    }
}
