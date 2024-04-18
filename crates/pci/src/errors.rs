use alloc::string::String;

pub type PciResult<T, E = PciError> = core::result::Result<T, E>;

#[derive(Debug, PartialEq)]
pub enum PciError {
    AddPciCap(u8, usize),
    AddPcieExtCap(u16, usize),
    UnregMemBar(usize),
    DeviceStatus(u32),
    PciRegister(u64),
    FeaturesSelect(u32),
    HotplugUnsupported(u8),
    InvalidConf(String, String),
    QueueEnable(u32),
    Other(String),
}

impl core::fmt::Display for PciError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PciError::AddPciCap(id, size) => write!(
                f,
                "Failed to add PCI capability: id 0x{:x}, size: 0x{:x}.",
                id, size
            ),
            PciError::AddPcieExtCap(id, size) => write!(
                f,
                "Failed to add PCIe extended capability: id 0x{:x}, size: 0x{:x}.",
                id, size
            ),
            PciError::UnregMemBar(index) => {
                write!(f, "Failed to unmap BAR {} in memory space.", index)
            }
            PciError::DeviceStatus(status) => write!(f, "Invalid device status 0x{:x}", status),
            PciError::PciRegister(reg) => write!(f, "Unsupported pci register, 0x{:x}", reg),
            PciError::FeaturesSelect(sel) => write!(f, "Invalid features select 0x{:x}", sel),
            PciError::HotplugUnsupported(devfn) => write!(
                f,
                "HotPlug is not supported for device with devfn {}",
                devfn
            ),
            PciError::InvalidConf(key, value) => {
                write!(f, "Invalid PCI configuration, key:{}, value:{}", key, value)
            }
            PciError::QueueEnable(value) => {
                write!(f, "Failed to enable queue, value is 0x{:x}", value)
            }
            PciError::Other(err) => write!(f, "{}", err),
        }
    }
}
