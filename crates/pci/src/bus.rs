use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};
use spin::Mutex;

use crate::config::BarAllocTrait;
use crate::{
    config::{
        Bar, BRIDGE_CONTROL, BRIDGE_CTL_SEC_BUS_RESET, SECONDARY_BUS_NUM, SUBORDINATE_BUS_NUM,
    },
    MsiIrqManager, PciDevOps,
};
use hypercraft::{HyperError, HyperResult as Result, MmioOps, PciError, PioOps};

type DeviceBusInfo<B: BarAllocTrait> = (Arc<Mutex<PciBus<B>>>, Arc<Mutex<dyn PciDevOps<B>>>);

/// PCI bus structure.
pub struct PciBus<B: BarAllocTrait> {
    /// Bus name
    pub name: String,
    /// Devices attached to the bus.
    pub devices: BTreeMap<u8, Arc<Mutex<dyn PciDevOps<B>>>>,
    /// Child buses of the bus.
    pub child_buses: Vec<Arc<Mutex<PciBus<B>>>>,
    /// Pci bridge which the bus originates from.
    pub parent_bridge: Option<Weak<Mutex<dyn PciDevOps<B>>>>,
    /// MSI interrupt manager.
    pub msi_irq_manager: Option<Arc<dyn MsiIrqManager>>,
}

impl<B: BarAllocTrait> PciBus<B> {
    /// Create new bus entity.
    ///
    /// # Arguments
    ///
    /// * `name` - String name of pci bus.
    pub fn new(name: String, msi_irq_manager: Option<Arc<dyn MsiIrqManager>>) -> Self {
        Self {
            name,
            devices: BTreeMap::new(),
            child_buses: Vec::new(),
            parent_bridge: None,
            msi_irq_manager,
        }
    }

    /// Get secondary bus number / subordinary bus number of the bus
    /// from configuration space of parent.
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset of bus number register.
    pub fn number(&self, offset: usize) -> u8 {
        let mut data = vec![0_u8; 1];
        self.get_bridge_control_reg(offset, &mut data);

        data[0]
    }

    /// Get device by the bdf.
    ///
    /// # Arguments
    ///
    /// * `bus_num` - The bus number.
    /// * `devfn` - Slot number << 8 | device number.
    pub fn get_device(&self, bus_num: u8, devfn: u8) -> Option<Arc<Mutex<dyn PciDevOps<B>>>> {
        if let Some(dev) = self.devices.get(&devfn) {
            // debug!("Find device {}:{}", bus_num, devfn);
            return Some((*dev).clone());
        }
        // debug!("Can't find device {}:{}", bus_num, devfn);
        None
    }

    /// Get which bar is mapped to the io_info_port.
    pub fn find_pio_bar(&self, port: u16) -> Option<Arc<Mutex<dyn PioOps>>> {
        for device in self.devices.values() {
            let device = device.lock();
            let pci_dev_base = device.pci_base().clone();
            let pci_config = &pci_dev_base.config;
            if let Some(bar) = pci_config.find_pio(port) {
                return Some(Arc::new(Mutex::new(bar.clone())));
            }
        }
        None
    }

    /// Get which bar is mapped to the mmio address.
    pub fn find_mmio_bar(&self, address: u64) -> Option<Arc<Mutex<dyn MmioOps>>> {
        for device in self.devices.values() {
            let device = device.lock();
            let pci_dev_base = device.pci_base().clone();
            let pci_config = &pci_dev_base.config;
            if let Some(bar) = pci_config.find_mmio(address) {
                return Some(Arc::new(Mutex::new(bar.clone())));
            }
        }
        None
    }

    fn in_range(&self, bus_num: u8) -> bool {
        if self.is_during_reset() {
            return false;
        }

        let secondary_bus_num: u8 = self.number(SECONDARY_BUS_NUM as usize);
        let subordinate_bus_num: u8 = self.number(SUBORDINATE_BUS_NUM as usize);
        if bus_num > secondary_bus_num && bus_num <= subordinate_bus_num {
            return true;
        }
        false
    }

    /// Find bus by the bus number.
    ///
    /// # Arguments
    ///
    /// * `bus` - Bus to find from.
    /// * `bus_number` - The bus number.
    pub fn find_bus_by_num(bus: &Arc<Mutex<Self>>, bus_num: u8) -> Option<Arc<Mutex<Self>>> {
        let locked_bus = bus.lock();
        if locked_bus.number(SECONDARY_BUS_NUM as usize) == bus_num {
            return Some((*bus).clone());
        }
        if locked_bus.in_range(bus_num) {
            for sub_bus in &locked_bus.child_buses {
                if let Some(b) = PciBus::find_bus_by_num(sub_bus, bus_num) {
                    return Some(b);
                }
            }
        }
        None
    }

    /// Find bus by name.
    ///
    /// # Arguments
    ///
    /// * `bus` - Bus to find from.
    /// * `name` - Bus name.
    pub fn find_bus_by_name(bus: &Arc<Mutex<Self>>, bus_name: &str) -> Option<Arc<Mutex<Self>>> {
        let locked_bus = bus.lock();
        if locked_bus.name.as_str() == bus_name {
            return Some((*bus).clone());
        }
        for sub_bus in &locked_bus.child_buses {
            if let Some(b) = PciBus::find_bus_by_name(sub_bus, bus_name) {
                return Some(b);
            }
        }
        None
    }

    /// Find the bus to which the device is attached.
    ///
    /// # Arguments
    ///
    /// * `pci_bus` - On which bus to find.
    /// * `name` - Device name.
    pub fn find_attached_bus(
        pci_bus: &Arc<Mutex<PciBus<B>>>,
        name: &str,
    ) -> Option<DeviceBusInfo<B>> {
        // Device is attached in pci_bus.
        let locked_bus = pci_bus.lock();
        for dev in locked_bus.devices.values() {
            if dev.lock().name() == name {
                return Some((pci_bus.clone(), dev.clone()));
            }
        }
        // Find in child bus.
        for bus in &locked_bus.child_buses {
            if let Some(found) = PciBus::find_attached_bus(bus, name) {
                return Some(found);
            }
        }
        None
    }

    /// Detach device from the bus.
    ///
    /// # Arguments
    ///
    /// * `bus` - Bus to detach from.
    /// * `dev` - Device attached to the bus.
    pub fn detach_device(bus: &Arc<Mutex<Self>>, dev: &Arc<Mutex<dyn PciDevOps<B>>>) -> Result<()> {
        let mut dev_locked = dev.lock();
        dev_locked.unrealize().map_err(|_err| {
            HyperError::PciError(PciError::Other(format!(
                "Failed to unrealize device {}",
                dev_locked.name()
            )))
        })?;

        let devfn = dev_locked.pci_base().devfn;
        let mut locked_bus = bus.lock();
        if locked_bus.devices.get(&devfn).is_some() {
            locked_bus.devices.remove(&devfn);
        } else {
            error!("Device {} not found in the bus", dev_locked.name());
        }

        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        for (_id, pci_dev) in self.devices.iter() {
            pci_dev.lock().reset(false).map_err(|_err| {
                HyperError::PciError(PciError::Other(format!("Fail to reset pci dev")))
            })?;
        }

        for child_bus in self.child_buses.iter_mut() {
            child_bus.lock().reset().map_err(|_err| {
                HyperError::PciError(PciError::Other(format!("Fail to reset child bus")))
            })?;
        }

        Ok(())
    }

    fn is_during_reset(&self) -> bool {
        let mut data = vec![0_u8; 2];
        self.get_bridge_control_reg(BRIDGE_CONTROL as usize + 1, &mut data);
        if data[1] & ((BRIDGE_CTL_SEC_BUS_RESET >> 8) as u8) != 0 {
            return true;
        }
        false
    }

    fn get_bridge_control_reg(&self, offset: usize, data: &mut [u8]) {
        if self.parent_bridge.is_none() {
            return;
        }

        self.parent_bridge
            .as_ref()
            .unwrap()
            .upgrade()
            .unwrap()
            .lock()
            .read_config(offset, data);
    }

    pub fn generate_dev_id(&self, devfn: u8) -> u16 {
        let bus_num = self.number(SECONDARY_BUS_NUM as usize);
        ((bus_num as u16) << 8) | (devfn as u16)
    }

    pub fn update_dev_id(&self, devfn: u8, dev_id: &Arc<AtomicU16>) {
        dev_id.store(self.generate_dev_id(devfn), Ordering::Release);
    }

    pub fn get_msi_irq_manager(&self) -> Option<Arc<dyn MsiIrqManager>> {
        match &self.parent_bridge {
            Some(parent_bridge) => {
                let parent_bridge = parent_bridge.upgrade().unwrap();
                let locked_parent_bridge = parent_bridge.lock();
                locked_parent_bridge.get_msi_irq_manager()
            }
            None => self.msi_irq_manager.clone(),
        }
    }
}
