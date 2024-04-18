#![no_std]

#[macro_use]
extern crate log;
#[macro_use]
extern crate alloc;
extern crate hashbrown;

pub mod config;
pub mod host;
pub mod msix;
pub mod util;

mod bus;
// mod root_port;

pub use bus::PciBus;
pub use config::{PciConfig, INTERRUPT_PIN};
pub use host::PciHost;
pub use msix::*;

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use core::mem::size_of;
use spin::Mutex;

use byteorder::{ByteOrder, LittleEndian};

use crate::config::{HEADER_TYPE, HEADER_TYPE_MULTIFUNC, MAX_FUNC};
pub use crate::util::AsAny;
use hypercraft::HyperResult as Result;

// const BDF_FUNC_SHIFT: u8 = 3;
pub const PCI_SLOT_MAX: u8 = 32;
pub const PCI_PIN_NUM: u8 = 4;
pub const PCI_INTR_BASE: u8 = 32;

// according to pci3.0 3.2.2.3.2. Figure 3-2: Layout of CONFIG_ADDRESS Register
// pub const BUS_MASK: u32 = 0x00ff_0000;  // [23:16]
// pub const SLOT_MASK: u32 = 0x0000_f800; // [15:11]
// pub const FUNC_MASK: u32 = 0x0000_0700; // [10:8]
// pub const OFFSET_MASK: u32 = 0x00fc;    // [7:2]

/// Macros that write data in little endian.
macro_rules! le_write {
    ($name: ident, $func: ident, $type: tt) => {
        pub fn $name(buf: &mut [u8], offset: usize, data: $type) -> Result<()> {
            let data_len: usize = size_of::<$type>();
            let buf_len: usize = buf.len();
            if offset + data_len > buf_len {
                error!(
                    "Out-of-bounds write access: buf_len = {}, offset = {}, data_len = {}",
                    buf_len, offset, data_len
                );
            }
            LittleEndian::$func(&mut buf[offset..(offset + data_len)], data);
            Ok(())
        }
    };
}

le_write!(le_write_u16, write_u16, u16);
le_write!(le_write_u32, write_u32, u32);
le_write!(le_write_u64, write_u64, u64);

/// Macros that read data in little endian.
macro_rules! le_read {
    ($name: ident, $func: ident, $type: tt) => {
        pub fn $name(buf: &[u8], offset: usize) -> Result<$type> {
            let data_len: usize = size_of::<$type>();
            let buf_len: usize = buf.len();
            if offset + data_len > buf_len {
                error!(
                    "Out-of-bounds read access: buf_len = {}, offset = {}, data_len = {}",
                    buf_len, offset, data_len
                );
            }
            Ok(LittleEndian::$func(&buf[offset..(offset + data_len)]))
        }
    };
}

le_read!(le_read_u16, read_u16, u16);
le_read!(le_read_u32, read_u32, u32);
le_read!(le_read_u64, read_u64, u64);

// fn le_write_set_value_u16(buf: &mut [u8], offset: usize, data: u16) -> Result<()> {
//     let val = le_read_u16(buf, offset)?;
//     le_write_u16(buf, offset, val | data)
// }

// fn le_write_clear_value_u16(buf: &mut [u8], offset: usize, data: u16) -> Result<()> {
//     let val = le_read_u16(buf, offset)?;
//     le_write_u16(buf, offset, val & !data)
// }

fn pci_devfn(slot: u8, func: u8) -> u8 {
    ((slot & 0x1f) << 3) | (func & 0x07)
}

fn pci_slot(devfn: u8) -> u8 {
    devfn >> 3 & 0x1f
}

fn pci_func(devfn: u8) -> u8 {
    devfn & 0x07
}

pub fn pci_ext_cap_id(header: u32) -> u16 {
    (header & 0xffff) as u16
}

pub fn pci_ext_cap_ver(header: u32) -> u32 {
    (header >> 16) & 0xf
}

pub fn pci_ext_cap_next(header: u32) -> usize {
    ((header >> 20) & 0xffc) as usize
}

#[derive(Clone)]
pub struct PciDevBase {
    /// Name of this device
    pub id: String,
    /// Pci config space.
    pub config: PciConfig,
    /// Devfn.
    pub devfn: u8,
    /// Primary Bus.
    pub parent_bus: Weak<Mutex<PciBus>>,
}

pub trait PciDevOps: Send + AsAny {
    /// Get device name.
    fn name(&self) -> String;

    /// Get base property of pci device.
    fn pci_base(&self) -> &PciDevBase;

    /// Get mutable base property of pci device.
    fn pci_base_mut(&mut self) -> &mut PciDevBase;

    /// Init writable bit mask.
    fn init_write_mask(&mut self, is_bridge: bool) -> Result<()> {
        self.pci_base_mut().config.init_common_write_mask()?;
        if is_bridge {
            self.pci_base_mut().config.init_bridge_write_mask()?;
        }

        Ok(())
    }

    /// Init write-and-clear bit mask.
    fn init_write_clear_mask(&mut self, is_bridge: bool) -> Result<()> {
        self.pci_base_mut().config.init_common_write_clear_mask()?;
        if is_bridge {
            self.pci_base_mut().config.init_bridge_write_clear_mask()?;
        }

        Ok(())
    }

    /// Realize PCI/PCIe device.
    fn realize(self) -> Result<()>;

    /// Unrealize PCI/PCIe device.
    fn unrealize(&mut self) -> Result<()> {
        panic!("Unrealize of the pci device is not implemented");
    }

    /// Configuration space read.
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset in configuration space.
    /// * `data` - Data buffer for reading.
    fn read_config(&mut self, offset: usize, data: &mut [u8]) {
        self.pci_base_mut().config.read(offset, data);
    }

    /// Configuration space write.
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset in configuration space.
    /// * `data` - Data to write.
    fn write_config(&mut self, offset: usize, data: &[u8]);

    /// Set device id to send MSI/MSI-X.
    ///
    /// # Arguments
    ///
    /// * `bus_num` - Bus number.
    /// * `devfn` - Slot number << 8 | Function number.
    ///
    /// # Returns
    ///
    /// Device id to send MSI/MSI-X.
    fn set_dev_id(&self, bus_num: u8, devfn: u8) -> u16 {
        let bus_shift: u16 = 8;
        ((bus_num as u16) << bus_shift) | (devfn as u16)
    }

    /// Reset device
    fn reset(&mut self, _reset_child_device: bool) -> Result<()> {
        Ok(())
    }

    /// Get the path of the PCI bus where the device resides.
    fn get_parent_dev_path(&self, parent_bus: Arc<Mutex<PciBus>>) -> String {
        let locked_parent_bus = parent_bus.lock();
        let parent_dev_path = if locked_parent_bus.name.eq("pcie.0") {
            String::from("/pci@ffffffffffffffff")
        } else {
            // This else branch will not be executed currently,
            // which is mainly to be compatible with new PCI bridge devices.
            // unwrap is safe because pci bus under root port will not return null.
            locked_parent_bus
                .parent_bridge
                .as_ref()
                .unwrap()
                .upgrade()
                .unwrap()
                .lock()
                .get_dev_path()
                .unwrap()
        };
        parent_dev_path
    }

    /// Fill the device path according to parent device path and device function.
    fn populate_dev_path(&self, parent_dev_path: String, devfn: u8, dev_type: &str) -> String {
        let slot = pci_slot(devfn);
        let function = pci_func(devfn);

        let slot_function = if function != 0 {
            format!("{:x},{:x}", slot, function)
        } else {
            format!("{:x}", slot)
        };

        format!("{}{}{}", parent_dev_path, dev_type, slot_function)
    }

    /// Get firmware device path.
    fn get_dev_path(&self) -> Option<String> {
        None
    }

    fn change_irq_level(&self, _irq_pin: u32, _level: i8) -> Result<()> {
        Ok(())
    }

    // fn get_intx_state(&self) -> Option<Arc<Mutex<PciIntxState>>> {
    //     None
    // }

    fn get_msi_irq_manager(&self) -> Option<Arc<dyn MsiIrqManager>> {
        None
    }
}

/// Init multifunction for pci devices.
///
/// # Arguments
///
/// * `multifunction` - Whether to open multifunction.
/// * `config` - Configuration space of pci devices.
/// * `devfn` - Devfn number.
/// * `parent_bus` - Parent bus of pci devices.
pub fn init_multifunction(
    multifunction: bool,
    config: &mut [u8],
    devfn: u8,
    parent_bus: Weak<Mutex<PciBus>>,
) -> Result<()> {
    let mut header_type =
        le_read_u16(config, HEADER_TYPE as usize)? & (!HEADER_TYPE_MULTIFUNC as u16);
    if multifunction {
        header_type |= HEADER_TYPE_MULTIFUNC as u16;
    }
    le_write_u16(config, HEADER_TYPE as usize, header_type)?;

    // Allow two ways of multifunction bit:
    // 1. The multifunction bit of all devices must be set;
    // 2. Function 0 must set the bit, the rest function (1~7) is allowed to
    // leave the bit to 0.
    let slot = pci_slot(devfn);
    let bus = parent_bus.upgrade().unwrap();
    let locked_bus = bus.lock();
    if pci_func(devfn) != 0 {
        let pci_dev = locked_bus.devices.get(&pci_devfn(slot, 0));
        if pci_dev.is_none() {
            return Ok(());
        }

        let mut data = vec![0_u8; 2];
        pci_dev
            .unwrap()
            .lock()
            .read_config(HEADER_TYPE as usize, data.as_mut_slice());
        if LittleEndian::read_u16(&data) & HEADER_TYPE_MULTIFUNC as u16 == 0 {
            // Function 0 should set multifunction bit.
            error!(
                "PCI: single function device can't be populated in bus {} function {}.{}",
                &locked_bus.name,
                slot,
                devfn & 0x07
            );
        }
        return Ok(());
    }

    if multifunction {
        return Ok(());
    }

    // If function 0 is set to single function, the rest function should be None.
    for func in 1..MAX_FUNC {
        if locked_bus.devices.get(&pci_devfn(slot, func)).is_some() {
            error!(
                "PCI: {}.0 indicates single function, but {}.{} is already populated",
                slot, slot, func
            );
        }
    }
    Ok(())
}

pub trait MsiIrqManager: Send + Sync {
    fn trigger(&self, _vector: MsiVector, _dev_id: u32) -> Result<()> {
        Ok(())
    }
}
