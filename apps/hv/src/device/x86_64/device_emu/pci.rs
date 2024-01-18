use super::PortIoDevice;
use bit_field::BitField;
use libax::hv::{Result as HyperResult, Error as HyperError};

pub struct PCIConfigurationSpace {
    port_base: u16,
    port_range: u16,
    current_address: u64,
}

const CONFIGURATION_SPACE_ADDRESS_PORT_OFFSET: usize = 0;
const CONFIGURATION_SPACE_ADDRESS_PORT_LAST_OFFSET: usize = 3;
const CONFIGURATION_SPACE_DATA_PORT_OFFSET: usize = 4;
const CONFIGURATION_SPACE_DATA_PORT_LAST_OFFSET: usize = 7;

impl PCIConfigurationSpace {
    pub fn new(port_base: u16, port_range: u16) -> Self {
        Self { port_base, port_range, current_address: 0 }
    }
}

impl PortIoDevice for PCIConfigurationSpace {
    fn port_range(&self) -> core::ops::Range<u16> {
        // return self.port_base..self.port_base + 8
        return self.port_base..self.port_base + self.port_range
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        match (port - self.port_base) as usize {
            offset @ CONFIGURATION_SPACE_ADDRESS_PORT_OFFSET ..= CONFIGURATION_SPACE_ADDRESS_PORT_LAST_OFFSET => {
                // we return non-sense to tell linux pci is not present.
                match access_size {
                    1 => Ok(0xfe),
                    2 => Ok(0xfffe),
                    4 => Ok(0xffff_fffe),
                    _ => Err(HyperError::InvalidParam),
                }
            },
            CONFIGURATION_SPACE_DATA_PORT_OFFSET ..= CONFIGURATION_SPACE_DATA_PORT_LAST_OFFSET => {
                match access_size {
                    1 => Ok(0xff),
                    2 => Ok(0xffff),
                    4 => Ok(0xffff_ffff),
                    _ => Err(HyperError::InvalidParam),
                }
            },
            _ => Err(HyperError::InvalidParam),
        }
    }

    fn write(&mut self, port: u16, access_size: u8, value: u32) -> HyperResult {
        match (port - self.port_base) as usize {
            offset @ CONFIGURATION_SPACE_ADDRESS_PORT_OFFSET..=CONFIGURATION_SPACE_ADDRESS_PORT_LAST_OFFSET => {
                match access_size {
                    1 => Ok({ self.current_address.set_bits(offset*8..offset*8+8, value as u8 as u64); }),
                    2 => Ok({ self.current_address.set_bits(offset*8..offset*8+16, value as u16 as u64); }),
                    4 => Ok({ self.current_address.set_bits(offset*8..offset*8+32, value as u64); }),
                    _ => Err(HyperError::InvalidParam),
                }
            },
            _ => Err(HyperError::NotSupported),
        }
    }
}
