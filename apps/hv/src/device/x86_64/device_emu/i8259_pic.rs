//! Emulated Intel 8259 Programmable Interrupt Controller. (ref: https://wiki.osdev.org/8259_PIC)

use super::PortIoDevice;
use bit_field::BitField;
use libax::hv::{Result as HyperResult, Error as HyperError};

pub struct I8259Pic {
    port_base: u16,
    port_range: u16,
    icw1: u8,
    offset: u8,
    icw3: u8,
    icw4: u8,
    icw_written: u8,
    icw_left: bool,
    mask: u8,
}

impl PortIoDevice for I8259Pic {
    fn port_range(&self) -> core::ops::Range<u16> {
        // self.port_base..self.port_base + 2
        self.port_base..self.port_base + self.port_range
    }

    fn read(&mut self, port: u16, _access_size: u8) -> HyperResult<u32> {
        // debug!("reading from pic port {port:#x}");
        match port - self.port_base {
            1 => Ok(self.mask as u32),
            _ => Err(HyperError::NotSupported),
        }
    }

    fn write(&mut self, port: u16, _access_size: u8, value: u32) -> HyperResult {
        // debug!("writing to pic port {port:#x}: {value:#x}");
        
        let value = value as u8;
        match port - self.port_base {
            0 => {
                if value.get_bit(4) {
                    self.icw1 = value;
                    self.icw_left = true;
                    self.icw_written = 1;
                } else {
                    // debug!("pit ocw ignored");
                }
            },
            1 => {
                if !self.icw_left {
                    self.mask = value;
                } else {
                    match self.icw_written {
                        1 => self.offset = value,
                        2 => self.icw3 = value,
                        3 => self.icw4 = value,
                        _ => return Err(HyperError::BadState),
                    }

                    if self.icw_written == 3 || (self.icw_written == 2 && !self.icw1.get_bit(0)) {
                        self.icw_left = false;
                        self.icw_written = 0;
                    } else {
                        self.icw_written += 1;
                    }
                }
            },
            _ => return Err(HyperError::InvalidParam),
        }

        Ok(()) // ignore write
    }
}

impl I8259Pic {
    pub const fn new(port_base: u16, port_range: u16) -> Self {
        Self {
            port_base,
            port_range,
            icw1: 0,
            offset: 0,
            icw3: 0,
            icw4: 0,
            icw_left: false,
            icw_written: 0,
            mask: 0,
        }
    }

    pub const fn mask(&self) -> u8 {
        self.mask
    }
}
