// Copyright (c) 2020 Huawei Technologies Co.,Ltd. All rights reserved.
//
// StratoVirt is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan
// PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//         http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
// KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// NON-INFRINGEMENT, MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.

use alloc::sync::Arc;
use spin::Mutex;
use alloc::string::String;
use core::ops::Range;

use crate::{bus::PciBus, PciDevOps, msix::MsiIrqManager};
#[cfg(target_arch = "x86_64")]
use crate::{le_read_u32, le_write_u32};

// use hypercraft::MmioOps;
#[cfg(target_arch = "x86_64")]
use hypercraft::PioOps;
use hypercraft::{HyperResult, HyperError};

#[cfg(target_arch = "x86_64")]
const CONFIG_ADDRESS_ENABLE_MASK: u32 = 0x8000_0000;
#[cfg(target_arch = "x86_64")]
const PIO_BUS_SHIFT: u32 = 16;
#[cfg(target_arch = "x86_64")]
const PIO_DEVFN_SHIFT: u32 = 8;
#[cfg(target_arch = "x86_64")]
const PIO_OFFSET_MASK: u32 = 0xff;

const CONFIG_BUS_MASK: u32 = 0xff;
const CONFIG_DEVFN_MASK: u32 = 0xff;
const ECAM_BUS_SHIFT: u32 = 20;
const ECAM_DEVFN_SHIFT: u32 = 12;
const ECAM_OFFSET_MASK: u64 = 0xfff;

const PCI_CFG_ADDR_PORT: Range<u16> = 0xcf8..0xcf8+4;
const PCI_CFG_DATA_PORT: Range<u16> = 0xcfc..0xcfc+4;

type GuestAddress = u64;
#[derive(Clone)]
pub struct PciHost {
    pub root_bus: Arc<Mutex<PciBus>>,
    #[cfg(target_arch = "x86_64")]
    config_addr: u32,
}

impl PciHost {
    /// Construct PCI/PCIe host.
    pub fn new(
        msi_irq_manager: Option<Arc<dyn MsiIrqManager>>,
    ) -> Self {
        // #[cfg(target_arch = "x86_64")]
        // let io_region = sys_io.root().clone();
        // let mem_region = sys_mem.root().clone();
        let root_bus = PciBus::new(
            String::from("pcie.0"),
            msi_irq_manager,
        );
        PciHost {
            root_bus: Arc::new(Mutex::new(root_bus)),
            #[cfg(target_arch = "x86_64")]
            config_addr: 0,
        }
    }

    pub fn find_device(&self, bus_num: u8, devfn: u8) -> Option<Arc<Mutex<dyn PciDevOps>>> {
        let locked_root_bus = self.root_bus.lock();
        if bus_num == 0 {
            return locked_root_bus.get_device(0, devfn);
        }
        for bus in &locked_root_bus.child_buses {
            if let Some(b) = PciBus::find_bus_by_num(bus, bus_num) {
                return b.lock().get_device(bus_num, devfn);
            }
        }
        None
    }
}

impl PioOps for PciHost {
    fn port_range(&self) -> Range<u16> {
        PCI_CFG_ADDR_PORT.start..PCI_CFG_DATA_PORT.end
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        let mut data = [0xffu8; 4];  // max access size is 4
        let cloned_hb = self.clone();
        if PCI_CFG_ADDR_PORT.contains(&port) {
            // Read configuration address register.
            if port!=PCI_CFG_ADDR_PORT.start || access_size != 4 {
                return Err(HyperError::InValidPioRead);
            }
            le_write_u32(&data, 0, cloned_hb.lock().unwrap().config_addr).unwrap();
        } else {    
            // Read configuration data register.
            let locked_hb = cloned_hb.lock().unwrap();
            if access_size > 4 || locked_hb.config_addr & CONFIG_ADDRESS_ENABLE_MASK == 0 {
               return Err(HyperError::InValidPioRead); 
            }

            let mut offset: u32 =
                (locked_hb.config_addr & !CONFIG_ADDRESS_ENABLE_MASK) + (port-PCI_CFG_DATA_PORT) as u32;
            let bus_num = ((offset >> PIO_BUS_SHIFT) & CONFIG_BUS_MASK) as u8;
            let devfn = ((offset >> PIO_DEVFN_SHIFT) & CONFIG_DEVFN_MASK) as u8;
            match locked_hb.find_device(bus_num, devfn) {
                Some(dev) => {
                    offset &= PIO_OFFSET_MASK;
                    dev.lock().unwrap().read_config(offset as usize, data);
                }
                None => {
                    for d in data.iter_mut() {
                        *d = 0xff;
                    }
                }
            }
        }
        match access_size {
            1 => Ok(u32::from_le_bytes([data[0], 0, 0, 0])),
            2 => Ok(u32::from_le_bytes([data[0], data[1], 0, 0])),
            4 => Ok(u32::from_le_bytes(data)),
            _ => Err(HyperError::InValidPioRead),
        }
    }

    fn write(&mut self, port: u16, access_size: u8, value: &[u8]) -> HyperResult {
        if PCI_CFG_ADDR_PORT.contains(&port) {
            // Write configuration address register.
            if port!=PCI_CFG_ADDR_PORT.start || access_size != 4 {
                return Err(HyperError::InValidPioWrite);
            }
            // save bdf for next read/write
            self.lock().unwrap().config_addr = le_read_u32(value, 0).unwrap();
        } else {    
            // Write configuration data register.
            let locked_hb = self.lock().unwrap();
            if access_size > 4 || locked_hb.config_addr & CONFIG_ADDRESS_ENABLE_MASK == 0 {
               return Err(HyperError::InValidPioWrite); 
            }

            let mut offset: u32 =
                (locked_hb.config_addr & !CONFIG_ADDRESS_ENABLE_MASK) + (port-PCI_CFG_DATA_PORT) as u32;
            let bus_num = ((offset >> PIO_BUS_SHIFT) & CONFIG_BUS_MASK) as u8;
            let devfn = ((offset >> PIO_DEVFN_SHIFT) & CONFIG_DEVFN_MASK) as u8;
            if let Some(dev) = locked_hb.find_device(bus_num, devfn) {
                offset &= PIO_OFFSET_MASK;
                dev.lock().unwrap().write_config(offset as usize, value);
            }
        }
        Ok(())
    }
}
