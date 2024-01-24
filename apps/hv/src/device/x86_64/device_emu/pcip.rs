use super::PortIoDevice;
use bit_field::BitField;
use libax::hv::{Result as HyperResult, Error as HyperError};
use x86::io;

use super::pci_dev::{
    BUS_MASK, SLOT_MASK, FUNC_MASK, OFFSET_MASK,
};

pub struct PCIPassthrough {
    port_base: u16,
    current_address: u64,
}

impl PCIPassthrough {
    pub fn new(port_base: u16) -> Self {
        Self { port_base, current_address: 0 }
    }
}

impl PortIoDevice for PCIPassthrough {
    fn port_range(&self) -> core::ops::Range<u16> {
        return self.port_base..self.port_base + 8
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        match access_size {
            // 1 => Ok(unsafe { io::inb(port) } as u32),
            // 2 => Ok(unsafe { io::inw(port) } as u32),
            // 4 => Ok(unsafe { io::inl(port) }),
            1 => {
                let value: u8;
                unsafe { value = io::inb(port) as _ };
                print!("[readb] this is passthrough pci read pci port:{:#x} access_size:{:#x} value:{:#x}\n", port, access_size, value);
                Ok(value as u32)
            },
            2 => {
                let value: u16;
                unsafe { value = io::inw(port) as _};
                print!("[readw] this is passthrough pci read pci port:{:#x} access_size:{:#x} value:{:#x}\n", port, access_size, value);
                Ok(value as u32)
            },
            4 => {
                let value: u32;
                unsafe { value = io::inl(port) as _};
                print!("[readl] this is passthrough pci read pci port:{:#x} access_size:{:#x} value:{:#x}\n", port, access_size, value);
                Ok(value)
            },
            _ => Err(HyperError::InvalidParam),
        }
    }

    fn write(&mut self, port: u16, access_size: u8, value: u32) -> HyperResult {
        /* 
        let bus = (value & BUS_MASK) >> 16;
        let slot = (value & SLOT_MASK) >> 11;
        let func = (value & FUNC_MASK) >> 8;
        let offset = value & OFFSET_MASK;
        print!("[write] pci port:{:#x} access_size:{:#x} val:{:#x} bus:{:#x} slot:{:#x} func:{:#x} offset:{:#x}\n", port, access_size, value, bus, slot, func, offset);
        */
        match access_size {
            1 => Ok(unsafe { io::outb(port, value as u8) }),
            2 => Ok(unsafe { io::outw(port, value as u16) }),
            4 => Ok(unsafe { io::outl(port, value) }),
            _ => Err(HyperError::InvalidParam),
        }
    }
}
