use crate::{from_hex, NAME_MAX_LENGTH, MAX_BASE_CNT};
use heapless::{String, Vec};

use core::fmt;
use serde::de::{self, Deserializer, Visitor};

const MAX_EMU_DEVICE_PER_VM: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmuDeviceType {
    EmuDevicePIC,
    EmuDevicePCI,
    EmuDeviceDebugPort,
    EmuDeviceUart16550,
    EmuDevicePit,
    EmuDeviceCmos,
    EmuDeviceDummy,
    EmuDeviceVirtialLocalApic,
    EmuDeviceApic,
}

struct EmuDeviceTypeVisitor;
impl<'de> Visitor<'de> for EmuDeviceTypeVisitor {
    type Value = EmuDeviceType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a EmuDeviceType")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        match value {
            "pic" => Ok(EmuDeviceType::EmuDevicePIC),
            "pci" => Ok(EmuDeviceType::EmuDevicePCI),
            "debug_port" => Ok(EmuDeviceType::EmuDeviceDebugPort),
            "uart16550" => Ok(EmuDeviceType::EmuDeviceUart16550),
            "apic" => Ok(EmuDeviceType::EmuDeviceApic),
            "virtual_apic" => Ok(EmuDeviceType::EmuDeviceVirtialLocalApic),
            "pit" => Ok(EmuDeviceType::EmuDevicePit),
            "cmos" => Ok(EmuDeviceType::EmuDeviceCmos),
            "dummy" => Ok(EmuDeviceType::EmuDeviceDummy),
            _ => Err(E::custom(alloc::format!(
                "unknown emu device type: {}",
                value
            ))),
        }
    }
}

pub fn from_emu_device_type<'de, D>(deserializer: D) -> Result<EmuDeviceType, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = deserializer.deserialize_str(EmuDeviceTypeVisitor);
    debug!("[from_emu_device_type] this is ret in visit str:{:?}", ret);
    ret
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeviceType {
    Pio,
    Msr,
}

struct DeviceTypeVisitor;
impl<'de> Visitor<'de> for DeviceTypeVisitor {
    type Value = DeviceType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a EmuDeviceType")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        match value {
            "pio" => Ok(DeviceType::Pio),
            "mmio" => Ok(DeviceType::Msr),
            _ => Err(E::custom(alloc::format!(
                "unknown emu device type: {}",
                value
            ))),
        }
    }
}

pub fn from_device_type<'de, D>(deserializer: D) -> Result<DeviceType, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = deserializer.deserialize_str(DeviceTypeVisitor);
    debug!("[from_io_type] this is ret in visit str:{:?}", ret);
    ret
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct VmEmulatedDeviceConfig {
    /// Emulated device name
    pub name: Option<String<NAME_MAX_LENGTH>>,
    /// Emulated device type
    #[serde(deserialize_with = "from_emu_device_type")]
    pub emu_type: EmuDeviceType,
    /// Device IO type
    #[serde(deserialize_with = "from_device_type")]
    pub device_type: DeviceType,
    /// Emulated device base ipa
    pub base: Vec<usize, MAX_BASE_CNT>,
    /// Emulated device io range
    pub range: Vec<usize, MAX_BASE_CNT>,
}

impl VmEmulatedDeviceConfig {
    pub fn new(
        name: Option<String<NAME_MAX_LENGTH>>,
        emu_type: EmuDeviceType,
        device_type: DeviceType,
        base: Vec<usize, MAX_BASE_CNT>,
        range: Vec<usize, MAX_BASE_CNT>,
    ) -> Self {
        Self {
            name: name,
            emu_type: emu_type,
            device_type: device_type,
            base: base,
            range: range,
        }
    }
}
#[derive(serde::Deserialize, Debug, Clone)]
pub struct VmEmulatedDeviceConfigList {
    pub emu_dev_list: Vec<VmEmulatedDeviceConfig, MAX_EMU_DEVICE_PER_VM>,
}

impl VmEmulatedDeviceConfigList {
    pub const fn default() -> VmEmulatedDeviceConfigList {
        VmEmulatedDeviceConfigList {
            emu_dev_list: Vec::new(),
        }
    }
    pub fn add_device_config(&mut self, device: VmEmulatedDeviceConfig) {
        self.emu_dev_list.push(device).unwrap();
    }
}
