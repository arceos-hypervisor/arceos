use crate::device::PioOps;

use crate::Result as HyperResult;

pub struct Dummy {
    port_base: u16,
    port_count: u16,
}

impl Dummy {
    pub fn new(port_base: u16, port_count: u16) -> Self {
        Self {
            port_base,
            port_count,
        }
    }
}

impl PioOps for Dummy {
    fn port_range(&self) -> core::ops::Range<u16> {
        self.port_base..self.port_base + self.port_count
    }

    fn read(&mut self, _port: u16, _access_size: u8) -> HyperResult<u32> {
        Ok(0)
    }

    fn write(&mut self, _port: u16, _access_size: u8, _value: &[u8]) -> HyperResult {
        Ok(())
    }
}
