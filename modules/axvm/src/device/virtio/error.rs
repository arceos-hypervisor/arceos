#[derive(Debug)]
pub enum VirtioError {
    Io {
        source: std::io::Error,
    },
    Util {
        source: util::error::UtilError,
    },
    EventFdCreate,
    EventFdWrite,
    ThreadCreate(String),
    ChannelSend(String),
    QueueIndex(u16, u16),
    QueueDescInvalid,
    AddressOverflow(&'static str, u64, u64),
    DevConfigOverflow(u64, u64, u64),
    InterruptTrigger(&'static str, super::VirtioInterruptType),
    VhostIoctl(String),
    ElementEmpty,
    VirtQueueIsNone,
    VirtQueueNotEnabled(String, usize),
    IncorrectQueueNum(usize, usize),
    IncorrectOffset(u64, u64),
    DeviceNotActivated(String),
    FailedToWriteConfig,
    ReadObjectErr(&'static str, u64),
    DevStatErr(u32),
    MmioRegErr(u64),
}

impl core::fmt::Display for VirtioError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VirtioError::Io { source } => write!(f, "Io error: {}", source),
            VirtioError::Util { source } => write!(f, "Util error: {}", source),
            VirtioError::EventFdCreate => write!(f, "Failed to create eventfd."),
            VirtioError::EventFdWrite => write!(f, "Failed to write eventfd."),
            VirtioError::ThreadCreate(name) => write!(f, "Failed to create {} thread", name),
            VirtioError::ChannelSend(name) => write!(f, "Failed to send {} on the channel", name),
            VirtioError::QueueIndex(index, size) => write!(f, "Queue index {} invalid, queue size is {}", index, size),
            VirtioError::QueueDescInvalid => write!(f, "Vring descriptor is invalid"),
            VirtioError::AddressOverflow(name, address, offset) => write!(f, "Address overflows for {}, address: 0x{:x}, offset: {}", name, address, offset),
            VirtioError::DevConfigOverflow(offset, len, size) => write!(f, "Failed to r/w dev config space: overflows, offset {}, len {}, space size {}", offset, len, size),
            VirtioError::InterruptTrigger(name, int_type) => write!(f, "Failed to trigger interrupt for {}, int-type {:?}", name, int_type),
            VirtioError::VhostIoctl(name) => write!(f, "Vhost ioctl failed: {}", name),
            VirtioError::ElementEmpty => write!(f, "Failed to get iovec from element!"),
            VirtioError::VirtQueueIsNone => write!(f, "Virt queue is none!"),
            VirtioError::VirtQueueNotEnabled(dev, queue) => write!(f, "Device {} virt queue {} is not enabled!", dev, queue),
            VirtioError::IncorrectQueueNum(expected, got) => write!(f, "Cannot perform activate. Expected {} queue(s), got {}", expected, got),
            VirtioError::IncorrectOffset(expected, got) => write!(f, "Incorrect offset, expected {}, got {}", expected, got),
            VirtioError::DeviceNotActivated(name) => write!(f, "Device {} not activated", name),
            VirtioError::FailedToWriteConfig => write!(f, "Failed to write config"),
            VirtioError::ReadObjectErr(name, address) => write!(f, "Failed to read object for {}, address: 0x{:x}", name, address),
            VirtioError::DevStatErr(status) => write!(f, "Invalid device status: 0x{:x}.", status),
            VirtioError::MmioRegErr(offset) => write!(f, "Unsupported mmio register at offset 0x{:x}.", offset),
        }
    }
}