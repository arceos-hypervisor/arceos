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
    pub fn new(port_base: u16,port_range:u16) -> Self {
        Self { port_base,port_range, current_address: 0 }
    }
}

impl PortIoDevice for PCIConfigurationSpace {
    fn port_range(&self) -> core::ops::Range<u16> {
        return self.port_base..self.port_base + 8
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
/*
use crate::device::device_emu::pci_dev;

use super::PortIoDevice;
use bit_field::BitField;
use libax::hv::{Result as HyperResult, Error as HyperError};
use x86::io;
use x86_64::registers::debug;
use alloc::sync::Arc;

use vm_config::pci_dev::{
    PCIDevice,
    read_u8, read_u16, read_u32,
    write_u8, write_u16, write_u32,
};
use super::pci_dev::{
    PCI_DEVICES,
    BUS_MASK, SLOT_MASK, FUNC_MASK, OFFSET_MASK,
};

pub struct PCIConfigurationSpace {
    port_base: u16,
    port_range: u16,
    current_device: Option<u32>,
    current_offset: u8,
    current_address: u64,
}

const CONFIGURATION_SPACE_ADDRESS_PORT_OFFSET: usize = 0;
const CONFIGURATION_SPACE_ADDRESS_PORT_LAST_OFFSET: usize = 3;
const CONFIGURATION_SPACE_DATA_PORT_OFFSET: usize = 4;
const CONFIGURATION_SPACE_DATA_PORT_LAST_OFFSET: usize = 7;

const PCI_CONFIG_BASE: u16 = 0xcf8;
const PCI_DATA_BASE: u16 = 0xcfc;
const PCI_CONFIG_HEADER_END: usize = 0x40;

const BAR_WRITE_MASK0: u32 = !0x7ff;
const BAR_WRITE_MASK1: u32 = !0;


impl PCIConfigurationSpace {
    pub fn new(port_base: u16, port_range: u16) -> Self {
        Self { 
            port_base, 
            port_range, 
            current_device: None,
            current_offset: 0,
            current_address: 0 
        }
    }
}

impl PortIoDevice for PCIConfigurationSpace {
    fn port_range(&self) -> core::ops::Range<u16> {
        // return self.port_base..self.port_base + 8
        return self.port_base..self.port_base + self.port_range
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        debug!("[read] this is read pci port:{:#x} access_size:{:#x}", port, access_size);
        let data_range = CONFIGURATION_SPACE_DATA_PORT_OFFSET..CONFIGURATION_SPACE_DATA_PORT_LAST_OFFSET+1;
        if data_range.contains(&((port - self.port_base) as _)) {
            if let Some(value) = self.current_device {
                self.current_device = None;
                self.current_offset = 0;

                let bdf = value & 0x00ff_ff00;
                let bus = (bdf & BUS_MASK) >> 16;
                let slot = (bdf & SLOT_MASK) >> 11;
                let func = (bdf & FUNC_MASK) >> 8;
                if let Some(pci_device) = PCI_DEVICES.read().get(&bdf) {
                    let read_value: u32;
                    let offset: usize;
                    match access_size {
                        1 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = ((value as u16 & OFFSET_MASK) + ((port - PCI_DATA_BASE) & 0b11)) as usize;
                            // capabilitiy list
                            if offset>=PCI_CONFIG_HEADER_END {
                                if let Some(capability) = pci_device.find_capability(offset as u8) {
                                    let capability_start = pci_device.find_capability_range(offset as u8).0 as usize;
                                    let capability_offset = offset - capability_start;
                                    info!("[read capability] this is read pci port:{:#x} offset:{:#x} base start:{:#x}", port, capability_offset, capability_start);
                                    read_value = read_u8(&capability, capability_offset) as u32;
                                }else {
                                    read_value = 0xff;
                                    warn!("[read capability] {:#x} not found", offset);
                                }
                            }else {
                                read_value = read_u8(pci_device, offset) as u32;
                            }
                        }
                        2 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = ((value as u16 & OFFSET_MASK) + ((port - PCI_DATA_BASE) & 0b10)) as usize;
                            if offset>=PCI_CONFIG_HEADER_END {
                                if let Some(capability) = pci_device.find_capability(offset as u8) {
                                    let capability_start = pci_device.find_capability_range(offset as u8).0 as usize;
                                    let capability_offset = offset - capability_start;
                                    info!("[read capability] this is read pci port:{:#x} offset:{:#x} base start:{:#x}", port, capability_offset, capability_start);
                                    read_value = read_u16(&capability, capability_offset) as u32;
                                }else {
                                    read_value = 0xffff;
                                    warn!("[read] capability {:#x} not found", offset);
                                }
                            }else {
                                read_value = read_u16(pci_device, offset) as u32;
                            }
                        }
                        4 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = (value as u16 & OFFSET_MASK) as usize;
                            // capabilitiy list
                            if offset>=PCI_CONFIG_HEADER_END {
                                if let Some(capability) = pci_device.find_capability(offset as u8) {
                                    let capability_start = pci_device.find_capability_range(offset as u8).0 as usize;
                                    let capability_offset = offset - capability_start;
                                    info!("[read capability] this is read pci port:{:#x} offset:{:#x} base start:{:#x}", port, capability_offset, capability_start);
                                    read_value = read_u32(&capability, capability_offset);
                                }else {
                                    read_value = 0xffff_ffff;
                                    warn!("[read] capability {:#x} not found", offset);
                                }
                            }else {
                                
                                read_value = read_u32(pci_device, offset);
                            }
                        }
                        _ => {return Err(HyperError::InvalidParam);}
                    }
                    info!("[read] this is read pci port:{:#x} access_size:{:#x} offset:{:#x} read_value: {:#x}", port, access_size, offset, read_value);
                    return Ok(read_value);
                }else {
                    warn!("[read] pci {:#x}:{:#x}:{:#x} device not found", bus, slot, func);
                }
            }
            match access_size {
                1 => {Ok(0xff)}
                2 => Ok(0xffff),
                4 => Ok(0xffff_ffff),
                _ => Err(HyperError::InvalidParam),
            }
        }else {
            info!("[not_read] this is read pci no emulated port:{:#x} access_size:{:#x}", port, access_size);
            // do not know why. according to arch/x86/pci/direct.c pci_check_type1()
            if port == PCI_CONFIG_BASE && access_size == 4{
                return Ok(0x80000000);
            }
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
                _ => Err(HyperError::InvalidParam),
            }
        }
    }

    fn write(&mut self, port: u16, access_size: u8, write_value: u32) -> HyperResult {
        // 0xcf8 set operation device, access size always 4?
        if port == PCI_CONFIG_BASE {
            // for debug usage
            let bus = (write_value & BUS_MASK) >> 16;
            let slot = (write_value & SLOT_MASK) >> 11;
            let func = (write_value & FUNC_MASK) >> 8;
            let offset = (write_value as u16 & OFFSET_MASK) as u8;
            info!("[write config] port:{:#x} write_value:{:#x} access_size:{:#x} bus:{:#x} slot:{:#x} func:{:#x} offset:{:#x}", port, write_value, access_size, bus, slot, func, offset);

            // let bdf = value & 0x00ff_ff00;
            self.current_device = Some(write_value);
            self.current_offset = offset;
            return Ok(());
        }else {
            if let Some(value) = self.current_device {
                self.current_device = None;
                self.current_offset = 0;

                let bdf = value & 0x00ff_ff00;
                let bus = (bdf & BUS_MASK) >> 16;
                let slot = (bdf & SLOT_MASK) >> 11;
                let func = (bdf & FUNC_MASK) >> 8;
                if let Some(pci_device) = PCI_DEVICES.write().get_mut(&bdf) {
                    let offset: usize;
                    match access_size {
                        1 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = ((value as u16 & OFFSET_MASK) + ((port - PCI_DATA_BASE) & 0b11)) as usize;
                            // capabilitiy list
                            if offset>=PCI_CONFIG_HEADER_END {
                                if let Some(mut capability) = pci_device.find_capability(offset as u8) {
                                    let (start, end) = pci_device.find_capability_range(offset as u8);
                                    let capability_offset = offset - start as usize;
                                    info!("[write data] pci port:{:#x} access_size:{:#x} offset:{:#x} capability_start:{:#x} write_value: {:#x}", port, access_size, offset, start, write_value);
                                    write_u8(&mut capability, capability_offset, write_value as u8);
                                    pci_device.update_capability_map(start, end, capability)
                                }else {
                                    warn!("[read capability] {:#x} not found", offset);
                                }
                            }else {
                                write_u8(pci_device, offset as usize, write_value as u8);
                            }
                        }
                        2 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = ((value as u16 & OFFSET_MASK) + ((port - PCI_DATA_BASE) & 0b10)) as usize;
                            
                            // capabilitiy list
                            if offset>=PCI_CONFIG_HEADER_END {
                                if let Some(mut capability) = pci_device.find_capability(offset as u8) {
                                    let (start, end) = pci_device.find_capability_range(offset as u8);
                                    let capability_offset = offset - start as usize;
                                    info!("[write data] pci port:{:#x} access_size:{:#x} offset:{:#x} capability_start:{:#x} write_value: {:#x}", port, access_size, offset, start, write_value);
                                    write_u16(&mut capability, capability_offset, write_value as u16);
                                    pci_device.update_capability_map(start, end, capability)
                                }else {
                                    warn!("[read capability] {:#x} not found", offset);
                                }
                            }else {
                                write_u16(pci_device, offset as usize, write_value as u16);
                            }
                        }
                        4 => {
                            // config space offset  according to arch/x86/pci/direct.c and early.c
                            offset = (value as u16 & OFFSET_MASK) as usize;
                            info!("[write data] pci port:{:#x} access_size:{:#x} offset:{:#x} read_value: {:#x}", port, access_size, offset, write_value);
                            if offset >= PCI_CONFIG_HEADER_END {
                                if let Some(mut capability) = pci_device.find_capability(offset as u8) {
                                    let (start, end) = pci_device.find_capability_range(offset as u8);
                                    let capability_offset = offset - start as usize;
                                    info!("[write data] pci port:{:#x} access_size:{:#x} offset:{:#x} capability_start:{:#x} write_value: {:#x}", port, access_size, offset, start, write_value);
                                    write_u32(&mut capability, capability_offset, write_value as u32);
                                    pci_device.update_capability_map(start, end, capability)
                                }else {
                                    warn!("[read capability] {:#x} not found", offset);
                                }
                            }
                            // update bar address to size???
                            else if (write_value==BAR_WRITE_MASK0 || write_value==BAR_WRITE_MASK1) 
                            && (0x10..=0x24).contains(&offset) {
                                let index = (offset - 0x10) / 4;
                                write_u32(pci_device, offset as usize, pci_device.bar_size[index]);
                            }else {
                                write_u32(pci_device, offset as usize, write_value as u32);
                            }
                        }
                        _ => {
                            return Err(HyperError::InvalidParam);
                        }
                    }
                    info!("[write data] pci port:{:#x} access_size:{:#x} offset:{:#x} read_value: {:#x}", port, access_size, offset, write_value);
                }else {
                    warn!("[write data] pci {:#x}:{:#x}:{:#x} device not found", bus, slot, func);
                }
            }
            return Ok(());
        }
        /* 
        match (port - self.port_base) as usize {
            offset @ CONFIGURATION_SPACE_ADDRESS_PORT_OFFSET..=CONFIGURATION_SPACE_ADDRESS_PORT_LAST_OFFSET => {
                match access_size {
                    // 1 => Ok({ self.current_address.set_bits(offset*8..offset*8+8, value as u8 as u64); }),
                    // 2 => Ok({ self.current_address.set_bits(offset*8..offset*8+16, value as u16 as u64); }),
                    // 4 => Ok({ self.current_address.set_bits(offset*8..offset*8+32, value as u64); }),
                    1 => Ok(()),
                    2 => Ok(()),
                    4 => Ok(()),
                    _ => Err(HyperError::InvalidParam),
                }
            },
            _ => Err(HyperError::NotSupported),
        }
        */
    }
}
 */