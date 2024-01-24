use crate::memory_config::VmMemoryRegion;
use crate::{from_hex, MAX_BASE_CNT};
use core::fmt;
use heapless::Vec;
use serde::de::{self, Deserializer, Visitor};
use super::pci_dev::PCIDevice;

const MAX_PASSTHROUGH_DEVICE_PER_VM: usize = 32;

pub const PCI_TYPE_DEVICE:u8 = 0x01;
pub const PCI_TYPE_BRIDGE:u8 = 0x02;
pub const MAX_PCI_DEVICE_MEM_REGION_CNT: usize = 4;

#[derive(Clone, Copy, Debug)]
pub enum PassthroughDeviceType {
    PCI,
    MMIO,
    PORT,
}

struct PassthroughDeviceTypeVisitor;
impl<'de> Visitor<'de> for PassthroughDeviceTypeVisitor {
    type Value = PassthroughDeviceType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a EmuPassthroughDeviceType")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        match value {
            "pci" => Ok(PassthroughDeviceType::PCI),
            "mmio" => Ok(PassthroughDeviceType::MMIO),
            "port" => Ok(PassthroughDeviceType::PORT),
            _ => Err(E::custom(alloc::format!(
                "unknown emu device type: {}",
                value
            ))),
        }
    }
}

pub fn from_device_type<'de, D>(deserializer: D) -> Result<PassthroughDeviceType, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = deserializer.deserialize_str(PassthroughDeviceTypeVisitor);
    ret
}


#[derive(Clone, Debug, serde::Deserialize)]
pub struct PortDevice {
    pub base: Vec<usize, MAX_BASE_CNT>,
    pub range: Vec<usize, MAX_BASE_CNT>,
}

#[derive(Clone, Debug)]
pub struct VmPassthroughDeviceConfig {
    pub mmio: Option<VmMemoryRegion>,
    pub pci: Option<PCIDevice>,
    pub port: Option<PortDevice>,
    // #[serde(deserialize_with = "from_device_type")]
    pub device_type: PassthroughDeviceType,
}

impl VmPassthroughDeviceConfig {
    pub fn new(
        device_type: PassthroughDeviceType,
        pci: Option<PCIDevice>,
        mmio: Option<VmMemoryRegion>,
        port: Option<PortDevice>,
    ) -> Self {
        match device_type {
            PassthroughDeviceType::MMIO => VmPassthroughDeviceConfig {
                mmio: mmio,
                pci: None,
                port: None,
                device_type: device_type,
            },
            PassthroughDeviceType::PCI => VmPassthroughDeviceConfig {
                mmio: None,
                pci: pci,
                port: None,
                device_type: device_type,
            },
            PassthroughDeviceType::PORT => VmPassthroughDeviceConfig {
                mmio: None,
                pci: None,
                port: port,
                device_type: device_type,
            },
            _ => panic!("Unknown device type"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VmPassthroughDeviceConfigList {
    pub passthrough_dev_list: Vec<VmPassthroughDeviceConfig, MAX_PASSTHROUGH_DEVICE_PER_VM>,
}

impl VmPassthroughDeviceConfigList {
    pub const fn default() -> VmPassthroughDeviceConfigList {
        VmPassthroughDeviceConfigList {
            passthrough_dev_list: Vec::new(),
        }
    }
    pub fn add_device_config(&mut self, device: VmPassthroughDeviceConfig) {
        self.passthrough_dev_list.push(device).unwrap();
    }
}
