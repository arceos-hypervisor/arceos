use alloc::boxed::Box;
use alloc::fmt::format;
use alloc::format;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::any::Any;
use core::cmp::{max, min};
use core::mem::size_of;
use core::sync::atomic::{AtomicU16, Ordering};
use lazy_static::lazy_static;
use spin::{mutex, rwlock::RwLock, Mutex};
use x86_64::registers::debug;

use byteorder::{ByteOrder, LittleEndian};

use crate::device::virtio::{
    virtio_has_feature, Queue, VirtioBaseState, VirtioDevice, VirtioInterrupt, VirtioInterruptType,
};
use crate::device::virtio::{
    CONFIG_STATUS_ACKNOWLEDGE, CONFIG_STATUS_DRIVER, CONFIG_STATUS_DRIVER_OK, CONFIG_STATUS_FAILED,
    CONFIG_STATUS_FEATURES_OK, CONFIG_STATUS_NEEDS_RESET, INVALID_VECTOR_NUM,
    QUEUE_TYPE_PACKED_VRING, QUEUE_TYPE_SPLIT_VRING, VIRTIO_F_RING_PACKED, VIRTIO_F_VERSION_1,
    VIRTIO_MMIO_INT_CONFIG, VIRTIO_MMIO_INT_VRING, VIRTIO_TYPE_BLOCK, VIRTIO_TYPE_CONSOLE,
    VIRTIO_TYPE_FS, VIRTIO_TYPE_GPU, VIRTIO_TYPE_NET, VIRTIO_TYPE_SCSI,
};
use hypercraft::{HyperError, HyperResult, MmioOps, PciError, PioOps, RegionOps, VirtioError};
use pci::config::{
    BarAllocTrait, RegionType, BAR_SPACE_UNMAPPED, DEVICE_ID, MINIMUM_BAR_SIZE_FOR_MMIO,
    PCIE_CONFIG_SPACE_SIZE, PCI_SUBDEVICE_ID_QEMU, PCI_VENDOR_ID_REDHAT_QUMRANET, REG_SIZE,
    REVISION_ID, STATUS, STATUS_INTERRUPT, SUBSYSTEM_ID, SUBSYSTEM_VENDOR_ID, SUB_CLASS_CODE,
    VENDOR_ID,
};
use pci::offset_of;
use pci::util::{
    byte_code::ByteCode,
    num_ops::{ranges_overlap, read_data_u32, write_data_u32, write_data_u64},
};
use pci::{
    config::{PciConfig, PCI_CAP_ID_VNDR, PCI_CAP_VNDR_AND_NEXT_SIZE},
    init_msix, init_multifunction, le_write_u16, le_write_u32, AsAny, PciBus, PciDevBase,
    PciDevOps,
};

const VIRTIO_QUEUE_MAX: u32 = 1024;

const VIRTIO_PCI_VENDOR_ID: u16 = PCI_VENDOR_ID_REDHAT_QUMRANET;
const VIRTIO_PCI_DEVICE_ID_BASE: u16 = 0x1040;
const VIRTIO_PCI_ABI_VERSION: u8 = 1;
const VIRTIO_PCI_CLASS_ID_NET: u16 = 0x0280;
const VIRTIO_PCI_CLASS_ID_BLOCK: u16 = 0x0100;
const VIRTIO_PCI_CLASS_ID_STORAGE_OTHER: u16 = 0x0180;
const VIRTIO_PCI_CLASS_ID_COMMUNICATION_OTHER: u16 = 0x0780;
const VIRTIO_PCI_CLASS_ID_DISPLAY_VGA: u16 = 0x0300;
const VIRTIO_PCI_CLASS_ID_OTHERS: u16 = 0x00ff;

const VIRTIO_PCI_CAP_COMMON_OFFSET: u32 = 0x0;
const VIRTIO_PCI_CAP_COMMON_LENGTH: u32 = 0x1000;
const VIRTIO_PCI_CAP_ISR_OFFSET: u32 = 0x1000;
const VIRTIO_PCI_CAP_ISR_LENGTH: u32 = 0x1000;
const VIRTIO_PCI_CAP_DEVICE_OFFSET: u32 = 0x2000;
const VIRTIO_PCI_CAP_DEVICE_LENGTH: u32 = 0x1000;
const VIRTIO_PCI_CAP_NOTIFY_OFFSET: u32 = 0x3000;
const VIRTIO_PCI_CAP_NOTIFY_LENGTH: u32 = 0x1000;
const VIRTIO_PCI_CAP_NOTIFY_END: u32 = 0x4000;
const VIRTIO_PCI_CAP_NOTIFY_OFF_MULTIPLIER: u32 = 4;

const VIRTIO_PCI_BAR_MAX: u8 = 3;
const VIRTIO_PCI_MSIX_BAR_IDX: u8 = 1;
const VIRTIO_PCI_MEM_BAR_IDX: u8 = 2;

/// Device (host) features set selector - Read Write.
const COMMON_DFSELECT_REG: u64 = 0x0;
/// Bitmask of the features supported by the device(host) (32 bits per set) - Read Only.
const COMMON_DF_REG: u64 = 0x4;
/// Driver (guest) features set selector - Read Write.
const COMMON_GFSELECT_REG: u64 = 0x8;
/// Bitmask of features activated by the driver (guest) (32 bits per set) - Write Only.
const COMMON_GF_REG: u64 = 0xc;
/// The configuration vector for MSI-X - Read Write.
const COMMON_MSIX_REG: u64 = 0x10;
/// The maximum number of virtqueues supported - Read Only.
const COMMON_NUMQ_REG: u64 = 0x12;
/// Device status - Read Write.
const COMMON_STATUS_REG: u64 = 0x14;
/// Configuration atomicity value - Read Only.
const COMMON_CFGGENERATION_REG: u64 = 0x15;
/// Queue selector - Read Write.
const COMMON_Q_SELECT_REG: u64 = 0x16;
/// The size for the currently selected queue - Read Write.
const COMMON_Q_SIZE_REG: u64 = 0x18;
/// The queue vector for MSI-X - Read Write.
const COMMON_Q_MSIX_REG: u64 = 0x1a;
/// Ready bit for the currently selected queue - Read Write.
const COMMON_Q_ENABLE_REG: u64 = 0x1c;
/// The offset from start of Notification structure at which this virtqueue is located - Read only
const COMMON_Q_NOFF_REG: u64 = 0x1e;
/// The low 32bit of queue's Descriptor Table address - Read Write.
const COMMON_Q_DESCLO_REG: u64 = 0x20;
/// The high 32bit of queue's Descriptor Table address - Read Write.
const COMMON_Q_DESCHI_REG: u64 = 0x24;
/// The low 32 bit of queue's Available Ring address - Read Write.
const COMMON_Q_AVAILLO_REG: u64 = 0x28;
/// The high 32 bit of queue's Available Ring address - Read Write.
const COMMON_Q_AVAILHI_REG: u64 = 0x2c;
/// The low 32bit of queue's Used Ring address - Read Write.
const COMMON_Q_USEDLO_REG: u64 = 0x30;
/// The high 32bit of queue's Used Ring address - Read Write.
const COMMON_Q_USEDHI_REG: u64 = 0x34;

/// The max features select num, only 0 or 1 is valid:
///   0: select feature bits 0 to 31.
///   1: select feature bits 32 to 63.
const MAX_FEATURES_SELECT_NUM: u32 = 2;

lazy_static! {
    pub static ref GLOBAL_VIRTIO_PCI_CFG_REQ: RwLock<Option<MmioReq>> = RwLock::new(None);
}

/// Virtio mmio req
#[derive(Clone, Debug)]
pub struct MmioReq {
    /// data
    pub data: Vec<u8>,
    /// access size
    pub len: u8,
    /// access address
    pub addr: u64,
    /// is write
    pub is_write: bool,
}

impl MmioReq {
    fn new(data: Vec<u8>, len: u8, addr: u64, is_write: bool) -> Self {
        MmioReq {
            data,
            len,
            addr,
            is_write,
        }
    }
}
/// Get class id according to device type.
///
/// # Arguments
///
/// * `device_type`  - Device type set by the host.
fn get_virtio_class_id(device_type: u32) -> u16 {
    match device_type {
        VIRTIO_TYPE_BLOCK => VIRTIO_PCI_CLASS_ID_BLOCK,
        VIRTIO_TYPE_SCSI => VIRTIO_PCI_CLASS_ID_BLOCK,
        VIRTIO_TYPE_FS => VIRTIO_PCI_CLASS_ID_STORAGE_OTHER,
        VIRTIO_TYPE_NET => VIRTIO_PCI_CLASS_ID_NET,
        VIRTIO_TYPE_CONSOLE => VIRTIO_PCI_CLASS_ID_COMMUNICATION_OTHER,
        #[cfg(target_arch = "x86_64")]
        VIRTIO_TYPE_GPU => VIRTIO_PCI_CLASS_ID_DISPLAY_VGA,
        _ => {
            warn!("Unknown device type, please make sure it is supported.");
            VIRTIO_PCI_CLASS_ID_OTHERS
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
enum VirtioPciCapType {
    Common = 1,
    Notify = 2,
    ISR = 3,
    Device = 4,
    CfgAccess = 5,
}

/// Virtio PCI Capability
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
struct VirtioPciCap {
    /// Capability length
    cap_len: u8,
    /// The type identifies the structure
    cfg_type: u8,
    /// The bar id where to find it
    bar_id: u8,
    /// Padding data
    padding: [u8; 3],
    /// Offset within bar
    offset: u32,
    /// Length of this structure, in bytes.
    length: u32,
}

impl ByteCode for VirtioPciCap {}

impl VirtioPciCap {
    fn new(cap_len: u8, cfg_type: u8, bar_id: u8, offset: u32, length: u32) -> Self {
        VirtioPciCap {
            cap_len,
            cfg_type,
            bar_id,
            padding: [0u8; 3],
            offset,
            length,
        }
    }
}

/// The struct of virtio pci capability for accessing BAR regions.
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
struct VirtioPciCfgAccessCap {
    /// The struct of virtio pci capability.
    cap: VirtioPciCap,
    /// Data for BAR regions access.
    pci_cfg_data: [u8; 4],
}

impl ByteCode for VirtioPciCfgAccessCap {}

impl VirtioPciCfgAccessCap {
    fn new(cap_len: u8, cfg_type: u8) -> Self {
        VirtioPciCfgAccessCap {
            cap: VirtioPciCap::new(cap_len, cfg_type, 0, 0, 0),
            pci_cfg_data: [0; 4],
        }
    }
}

/// The struct of virtio pci capability for notifying the host
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
struct VirtioPciNotifyCap {
    /// The struct of virtio pci capability
    cap: VirtioPciCap,
    /// Multiplier for queue_notify_off
    notify_off_multiplier: u32,
}

impl ByteCode for VirtioPciNotifyCap {}

impl VirtioPciNotifyCap {
    fn new(
        cap_len: u8,
        cfg_type: u8,
        bar_id: u8,
        offset: u32,
        length: u32,
        notify_off_multiplier: u32,
    ) -> Self {
        VirtioPciNotifyCap {
            cap: VirtioPciCap::new(cap_len, cfg_type, bar_id, offset, length),
            notify_off_multiplier,
        }
    }
}

/// Virtio-PCI device structure
#[derive(Clone)]
pub struct VirtioPciDevice<B: BarAllocTrait> {
    base: PciDevBase<B>,
    /// The entity of virtio device
    device: Arc<Mutex<dyn VirtioDevice>>,
    /// Device id
    dev_id: Arc<AtomicU16>,
    /// Offset of VirtioPciCfgAccessCap in Pci config space.
    cfg_cap_offset: usize,
    /// The function for interrupt triggering
    interrupt_cb: Option<Arc<VirtioInterrupt>>,
}

impl<B: BarAllocTrait + 'static> VirtioPciDevice<B> {
    pub fn new(
        name: String,
        devfn: u8,
        device: Arc<Mutex<dyn VirtioDevice>>,
        parent_bus: Weak<Mutex<PciBus<B>>>,
        multi_func: bool,
    ) -> Self {
        let queue_num = device.lock().queue_num();
        VirtioPciDevice {
            base: PciDevBase {
                id: name,
                config: PciConfig::<B>::new(PCIE_CONFIG_SPACE_SIZE, VIRTIO_PCI_BAR_MAX),
                devfn,
                parent_bus,
            },
            device,
            dev_id: Arc::new(AtomicU16::new(0)),
            cfg_cap_offset: 0,
            interrupt_cb: None,
        }
    }

    fn assign_interrupt_cb(&mut self) {
        let locked_dev = self.device.lock();
        let virtio_base = locked_dev.virtio_base();
        let device_status = virtio_base.device_status.clone();
        let interrupt_status = virtio_base.interrupt_status.clone();
        let msix_config = virtio_base.config_vector.clone();
        let config_generation = virtio_base.config_generation.clone();

        let cloned_msix = self.base.config.msix.as_ref().unwrap().clone();
        let dev_id = self.dev_id.clone();

        let cb = Arc::new(Box::new(
            move |int_type: &VirtioInterruptType, queue: Option<&Queue>, needs_reset: bool| {
                let vector = match int_type {
                    VirtioInterruptType::Config => {
                        if needs_reset {
                            device_status.fetch_or(CONFIG_STATUS_NEEDS_RESET, Ordering::SeqCst);
                        }
                        if device_status.load(Ordering::Acquire) & CONFIG_STATUS_DRIVER_OK == 0 {
                            return Ok(());
                        }

                        // Use (CONFIG | VRING) instead of CONFIG, it can be used to solve the
                        // IO stuck problem by change the device configure.
                        interrupt_status.fetch_or(
                            VIRTIO_MMIO_INT_CONFIG | VIRTIO_MMIO_INT_VRING,
                            Ordering::SeqCst,
                        );
                        config_generation.fetch_add(1, Ordering::SeqCst);
                        msix_config.load(Ordering::Acquire)
                    }
                    VirtioInterruptType::Vring => {
                        interrupt_status.fetch_or(VIRTIO_MMIO_INT_VRING, Ordering::SeqCst);
                        queue.map_or(0, |q| q.vring.get_queue_config().vector)
                    }
                };

                let mut locked_msix = cloned_msix.lock();
                if locked_msix.enabled {
                    locked_msix.notify(vector, dev_id.load(Ordering::Acquire));
                } else {
                    error!("MSI-X is not enabled, failed to notify interrupt.");
                }

                Ok(())
            },
        ) as VirtioInterrupt);

        self.interrupt_cb = Some(cb);
    }

    // add modern virtio device capability
    fn modern_mem_region_cap_add<T: ByteCode>(&mut self, data: T) -> HyperResult<usize> {
        let cap_offset = self.base.config.add_pci_cap(
            PCI_CAP_ID_VNDR,
            size_of::<T>() + PCI_CAP_VNDR_AND_NEXT_SIZE as usize,
        )?;

        let write_start = cap_offset + PCI_CAP_VNDR_AND_NEXT_SIZE as usize;
        self.base.config.config[write_start..(write_start + size_of::<T>())]
            .copy_from_slice(data.as_bytes());

        Ok(write_start)
    }

    fn activate_device(&self) -> bool {
        let mut locked_dev = self.device.lock();
        if locked_dev.device_activated() {
            return true;
        }

        let queue_type = locked_dev.queue_type();
        let features = locked_dev.virtio_base().driver_features;
        let broken = locked_dev.virtio_base().broken.clone();

        let mut queues = Vec::new();
        let queues_config = &mut locked_dev.virtio_base_mut().queues_config;
        for q_config in queues_config.iter_mut() {
            if !q_config.ready {
                debug!("queue is not ready, please check your init process");
            } else {
                q_config.set_addr_cache(self.interrupt_cb.clone().unwrap(), features, &broken);
            }
            let queue = Queue::new(*q_config, queue_type).unwrap();
            if q_config.ready && !queue.is_valid() {
                error!("Failed to activate device: Invalid queue");
                return false;
            }
            let arc_queue = Arc::new(Mutex::new(queue));
            queues.push(arc_queue.clone());
        }
        locked_dev.virtio_base_mut().queues = queues;

        let parent = self.base.parent_bus.upgrade().unwrap();
        parent.lock().update_dev_id(self.base.devfn, &self.dev_id);

        if let Err(e) = locked_dev.activate(self.interrupt_cb.clone().unwrap()) {
            error!("Failed to activate device, error is {:?}", e);
            return false;
        }

        locked_dev.set_device_activated(true);
        true
    }

    fn deactivate_device(&self) -> bool {
        let mut locked_dev = self.device.lock();
        if locked_dev.device_activated() {
            if let Err(e) = locked_dev.deactivate() {
                error!("Failed to deactivate virtio device, error is {:?}", e);
                return false;
            }
            locked_dev.virtio_base_mut().reset();
        }

        if let Some(msix) = &self.base.config.msix {
            msix.lock().clear_pending_vectors();
        }

        true
    }

    /// Read data from the common config of virtio device.
    /// Return the config value in u32.
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset of common config.
    /// struct virtio_pci_common_cfg {
    ///         /* About the whole device. */
    ///         le32 device_feature_select;     /* read-write */
    ///         le32 device_feature;            /* read-only for driver */
    ///         le32 driver_feature_select;     /* read-write */
    ///         le32 driver_feature;            /* read-write */
    ///         le16 config_msix_vector;        /* read-write */
    ///         le16 num_queues;                /* read-only for driver */
    ///         u8 device_status;               /* read-write */
    ///         u8 config_generation;           /* read-only for driver */

    ///        le16 queue_select;              /* read-write */
    ///         le16 queue_size;                /* read-write */
    ///         le16 queue_msix_vector;         /* read-write */
    ///         le16 queue_enable;              /* read-write */
    ///         le16 queue_notify_off;          /* read-only for driver */
    ///         le64 queue_desc;                /* read-write */
    ///         le64 queue_driver;              /* read-write */
    ///         le64 queue_device;              /* read-write */
    ///         le16 queue_notify_data;         /* read-only for driver */
    ///         le16 queue_reset;               /* read-write */
    /// };
    fn read_common_config(&self, offset: u64) -> HyperResult<u32> {
        let locked_device = self.device.lock();
        let value = match offset {
            COMMON_DFSELECT_REG => locked_device.hfeatures_sel(),
            COMMON_DF_REG => {
                let dfeatures_sel = locked_device.hfeatures_sel();
                if dfeatures_sel < MAX_FEATURES_SELECT_NUM {
                    locked_device.device_features(dfeatures_sel)
                } else {
                    0
                }
            }
            COMMON_GFSELECT_REG => locked_device.gfeatures_sel(),
            COMMON_GF_REG => {
                let gfeatures_sel = locked_device.gfeatures_sel();
                if gfeatures_sel < MAX_FEATURES_SELECT_NUM {
                    locked_device.driver_features(gfeatures_sel)
                } else {
                    0
                }
            }
            COMMON_MSIX_REG => locked_device.config_vector() as u32,
            COMMON_NUMQ_REG => locked_device.virtio_base().queues_config.len() as u32,
            COMMON_STATUS_REG => locked_device.device_status(),
            COMMON_CFGGENERATION_REG => locked_device.config_generation() as u32,
            COMMON_Q_SELECT_REG => locked_device.queue_select() as u32,
            COMMON_Q_SIZE_REG => locked_device
                .queue_config()
                .map(|config| u32::from(config.size))?,
            COMMON_Q_MSIX_REG => locked_device
                .queue_config()
                .map(|config| u32::from(config.vector))?,
            COMMON_Q_ENABLE_REG => locked_device
                .queue_config()
                .map(|config| u32::from(config.ready))?,
            COMMON_Q_NOFF_REG => locked_device.queue_select() as u32,
            COMMON_Q_DESCLO_REG => locked_device
                .queue_config()
                .map(|config| config.desc_table as u32)?,
            COMMON_Q_DESCHI_REG => locked_device
                .queue_config()
                .map(|config| (config.desc_table >> 32) as u32)?,
            COMMON_Q_AVAILLO_REG => locked_device
                .queue_config()
                .map(|config| config.avail_ring as u32)?,
            COMMON_Q_AVAILHI_REG => locked_device
                .queue_config()
                .map(|config| (config.avail_ring >> 32) as u32)?,
            COMMON_Q_USEDLO_REG => locked_device
                .queue_config()
                .map(|config| config.used_ring as u32)?,
            COMMON_Q_USEDHI_REG => locked_device
                .queue_config()
                .map(|config| (config.used_ring >> 32) as u32)?,
            _ => 0,
        };

        Ok(value)
    }

    /// Write data to the common config of virtio device.
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset of common config.
    /// * `value` - The value to write.
    ///
    /// # Errors
    ///
    /// Returns Error if the offset is out of bound.
    fn write_common_config(&mut self, offset: u64, value: u32) -> HyperResult<()> {
        let mut locked_device = self.device.lock();
        match offset {
            COMMON_DFSELECT_REG => {
                locked_device.set_hfeatures_sel(value);
            }
            COMMON_GFSELECT_REG => {
                locked_device.set_gfeatures_sel(value);
            }
            COMMON_GF_REG => {
                if locked_device.device_status() & CONFIG_STATUS_FEATURES_OK != 0 {
                    error!("it's not allowed to set features after having been negoiated");
                    return Ok(());
                }
                let gfeatures_sel = locked_device.gfeatures_sel();
                if gfeatures_sel >= MAX_FEATURES_SELECT_NUM {
                    return Err(HyperError::PciError(PciError::FeaturesSelect(
                        gfeatures_sel,
                    )));
                }
                locked_device.set_driver_features(gfeatures_sel, value);

                if gfeatures_sel == 1 {
                    let features = (locked_device.driver_features(1) as u64) << 32;
                    if virtio_has_feature(features, VIRTIO_F_RING_PACKED) {
                        locked_device.set_queue_type(QUEUE_TYPE_PACKED_VRING);
                    } else {
                        locked_device.set_queue_type(QUEUE_TYPE_SPLIT_VRING);
                    }
                }
            }
            COMMON_MSIX_REG => {
                if self.base.config.revise_msix_vector(value) {
                    locked_device.set_config_vector(value as u16);
                } else {
                    locked_device.set_config_vector(INVALID_VECTOR_NUM);
                }
                locked_device.set_interrupt_status(0);
            }
            COMMON_STATUS_REG => {
                if value & CONFIG_STATUS_FEATURES_OK != 0 && value & CONFIG_STATUS_DRIVER_OK == 0 {
                    let features = (locked_device.driver_features(1) as u64) << 32;
                    debug!("driver_features is {:#x}", features);
                    if !virtio_has_feature(features, VIRTIO_F_VERSION_1) {
                        error!(
                            "Device is modern only, but the driver not support VIRTIO_F_VERSION_1"
                        );
                        return Ok(());
                    }
                }
                if value != 0 && (locked_device.device_status() & !value) != 0 {
                    error!("Driver must not clear a device status bit");
                    return Ok(());
                }

                let old_status = locked_device.device_status();
                locked_device.set_device_status(value);
                if locked_device.check_device_status(
                    CONFIG_STATUS_ACKNOWLEDGE
                        | CONFIG_STATUS_DRIVER
                        | CONFIG_STATUS_DRIVER_OK
                        | CONFIG_STATUS_FEATURES_OK,
                    CONFIG_STATUS_FAILED,
                ) {
                    drop(locked_device);
                    self.activate_device();
                } else if old_status != 0 && locked_device.device_status() == 0 {
                    drop(locked_device);
                    self.deactivate_device();
                }
            }
            COMMON_Q_SELECT_REG => {
                if value < VIRTIO_QUEUE_MAX {
                    locked_device.set_queue_select(value as u16);
                }
            }
            COMMON_Q_SIZE_REG => locked_device
                .queue_config_mut(true)
                .map(|config| config.size = value as u16)?,
            COMMON_Q_ENABLE_REG => {
                if value != 1 {
                    error!("Driver set illegal value for queue_enable {}", value);
                    return Err(HyperError::PciError(PciError::QueueEnable(value)));
                }
                locked_device
                    .queue_config_mut(true)
                    .map(|config| config.ready = true)?;
            }
            COMMON_Q_MSIX_REG => {
                let val = if self.base.config.revise_msix_vector(value) {
                    value as u16
                } else {
                    INVALID_VECTOR_NUM
                };
                // It should not check device status when detaching device which
                // will set vector to INVALID_VECTOR_NUM.
                let need_check = locked_device.device_status() != 0;
                locked_device
                    .queue_config_mut(need_check)
                    .map(|config| config.vector = val)?;
            }
            COMMON_Q_DESCLO_REG => locked_device.queue_config_mut(true).map(|config| {
                config.desc_table = config.desc_table | u64::from(value);
            })?,
            COMMON_Q_DESCHI_REG => locked_device.queue_config_mut(true).map(|config| {
                config.desc_table = config.desc_table | (u64::from(value) << 32);
            })?,
            COMMON_Q_AVAILLO_REG => locked_device.queue_config_mut(true).map(|config| {
                config.avail_ring = config.avail_ring | u64::from(value);
            })?,
            COMMON_Q_AVAILHI_REG => locked_device.queue_config_mut(true).map(|config| {
                config.avail_ring = config.avail_ring | (u64::from(value) << 32);
            })?,
            COMMON_Q_USEDLO_REG => locked_device.queue_config_mut(true).map(|config| {
                config.used_ring = config.used_ring | u64::from(value);
            })?,
            COMMON_Q_USEDHI_REG => locked_device.queue_config_mut(true).map(|config| {
                config.used_ring = config.used_ring | (u64::from(value) << 32);
            })?,
            _ => {
                return Err(HyperError::PciError(PciError::PciRegister(offset)));
            }
        };

        Ok(())
    }

    // build pci cfg cap ops(common_cfg, isr_cfg, device_cfg, notify_cfg)
    fn build_pci_cfg_cap_ops(virtio_pci: Arc<Mutex<VirtioPciDevice<B>>>) -> RegionOps {
        let cloned_virtio_pci = virtio_pci.clone();
        let read = move |offset: u64, access_size: u8| -> HyperResult<u64> {
            let mut data = [0u8; 8];
            match offset as u32 {
                // read pci common cfg
                VIRTIO_PCI_CAP_COMMON_OFFSET..VIRTIO_PCI_CAP_ISR_OFFSET => {
                    debug!("read pci common cfg, offset is {:#x}", offset);
                    let common_offset = offset - VIRTIO_PCI_CAP_COMMON_OFFSET as u64;
                    let value = match cloned_virtio_pci.lock().read_common_config(common_offset) {
                        Ok(v) => v,
                        Err(e) => {
                            error!(
                                "Failed to read common config of virtio-pci device, error is {:?}",
                                e,
                            );
                            return Err(HyperError::InValidMmioRead);
                        }
                    };

                    write_data_u64(&mut data[..], value);
                }
                // read pci isr cfg
                VIRTIO_PCI_CAP_ISR_OFFSET..VIRTIO_PCI_CAP_DEVICE_OFFSET => {
                    debug!("read pci isr cfg, offset is {}", offset);
                    let cloned_virtio_dev = cloned_virtio_pci.lock().device.clone();
                    if let Some(val) = data.get_mut(0) {
                        let device_lock = cloned_virtio_dev.lock();
                        *val = device_lock
                            .virtio_base()
                            .interrupt_status
                            .swap(0, Ordering::SeqCst) as u8;
                    }
                }
                // read pci device cfg
                VIRTIO_PCI_CAP_DEVICE_OFFSET..VIRTIO_PCI_CAP_NOTIFY_OFFSET => {
                    debug!("read pci device cfg, offset is {}", offset);
                    let cloned_virtio_dev = cloned_virtio_pci.lock().device.clone();
                    let device_offset = offset - VIRTIO_PCI_CAP_DEVICE_OFFSET as u64;
                    if let Err(e) = cloned_virtio_dev
                        .lock()
                        .read_config(device_offset, &mut data[..])
                    {
                        error!("Failed to read virtio-dev config space, error is {:?}", e);
                        return Err(HyperError::InValidMmioRead);
                    };
                }
                // read pci notify cfg
                VIRTIO_PCI_CAP_NOTIFY_OFFSET..VIRTIO_PCI_CAP_NOTIFY_END => {
                    debug!("read pci notify cfg, offset is {}", offset);
                    // todo: need to notify hv to get the virtio request
                }
                _ => {
                    error!("Invalid offset for pci cfg cap, offset is {}", offset);
                    return Err(HyperError::InValidMmioRead);
                }
            };
            Ok(u64::from_le_bytes(data))
        };
        let cloned_virtio_pci = virtio_pci.clone();
        let write = move |offset: u64, access_size: u8, data: &[u8]| -> HyperResult {
            match offset as u32 {
                // write pci common cfg
                VIRTIO_PCI_CAP_COMMON_OFFSET..VIRTIO_PCI_CAP_ISR_OFFSET => {
                    debug!(
                        "write pci common cfg, write data:{:?} offset is {:#x}",
                        data, offset
                    );
                    let common_offset = offset - VIRTIO_PCI_CAP_COMMON_OFFSET as u64;
                    let mut value = 0;
                    if !read_data_u32(data, &mut value) {
                        return Err(HyperError::InValidMmioWrite);
                    }

                    if let Err(e) = cloned_virtio_pci
                        .lock()
                        .write_common_config(common_offset, value)
                    {
                        error!(
                            "Failed to write common config of virtio-pci device, error is {:?}",
                            e,
                        );
                        return Err(HyperError::InValidMmioWrite);
                    }
                }
                // write pci isr cfg
                VIRTIO_PCI_CAP_ISR_OFFSET..VIRTIO_PCI_CAP_DEVICE_OFFSET => {
                    debug!(
                        "write pci isr cfg, write data:{:?} offset is {:#x}",
                        data, offset
                    );
                }
                // write pci device cfg
                VIRTIO_PCI_CAP_DEVICE_OFFSET..VIRTIO_PCI_CAP_NOTIFY_OFFSET => {
                    debug!(
                        "write pci device cfg, write data:{:?} offset is {:#x}",
                        data, offset
                    );
                    let cloned_virtio_dev = cloned_virtio_pci.lock().device.clone();
                    let device_offset = offset - VIRTIO_PCI_CAP_DEVICE_OFFSET as u64;
                    if let Err(e) = cloned_virtio_dev
                        .lock()
                        .write_config(device_offset, &data[..])
                    {
                        error!("Failed to write virtio-dev config space, error is {:?}", e);
                        return Err(HyperError::InValidMmioWrite);
                    };
                }
                // write pci notify cfg
                VIRTIO_PCI_CAP_NOTIFY_OFFSET..VIRTIO_PCI_CAP_NOTIFY_END => {
                    debug!(
                        "write pci notify cfg, write data:{:?} offset is {:#x}",
                        data, offset
                    );
                    // todo: need to notify hv to get the virtio request
                }
                _ => {
                    error!("Invalid offset for pci cfg cap, offset is {:#x}", offset);
                    return Err(HyperError::InValidMmioRead);
                }
            };
            Ok(())
        };
        RegionOps {
            read: Arc::new(read),
            write: Arc::new(write),
        }
    }

    // Access virtio configuration through VirtioPciCfgAccessCap.
    fn do_cfg_access(&mut self, start: usize, end: usize, is_write: bool) -> Option<MmioReq> {
        let pci_cfg_data_offset =
            self.cfg_cap_offset + offset_of!(VirtioPciCfgAccessCap, pci_cfg_data);
        let cap_size = size_of::<VirtioPciCfgAccessCap>();
        // SAFETY: pci_cfg_data_offset is the offset of VirtioPciCfgAccessCap in Pci config space
        // which is much less than u16::MAX.
        if !ranges_overlap(start, end - start, pci_cfg_data_offset, cap_size).unwrap() {
            return None;
        }

        // pci config access cap
        let config = &self.base.config.config[self.cfg_cap_offset..];
        // access bar id
        let bar: u8 = config[offset_of!(VirtioPciCap, bar_id)];
        // offset of the bar
        let off = LittleEndian::read_u32(&config[offset_of!(VirtioPciCap, offset)..]);
        // access length
        let len = LittleEndian::read_u32(&config[offset_of!(VirtioPciCap, length)..]);
        if bar >= VIRTIO_PCI_BAR_MAX {
            warn!("The bar_id {} of VirtioPciCfgAccessCap exceeds max", bar);
            return None;
        }
        let bar_base = self.base.config.get_bar_address(bar as usize);
        // check bar access whether is valid
        if bar_base == BAR_SPACE_UNMAPPED {
            debug!("The bar {} of VirtioPciCfgAccessCap is not mapped", bar);
            return None;
        }
        if ![1, 2, 4].contains(&len) {
            debug!("The length {} of VirtioPciCfgAccessCap is illegal", len);
            return None;
        }
        if off & (len - 1) != 0 {
            warn!("The offset {} of VirtioPciCfgAccessCap is not aligned", off);
            return None;
        }
        if (off as u64)
            .checked_add(len as u64)
            .filter(|&end| end <= self.base.config.bars[bar as usize].size)
            .is_none()
        {
            warn!("The access range of VirtioPciCfgAccessCap exceeds bar size");
            return None;
        }
        let data = self.base.config.config[pci_cfg_data_offset..].as_ref();
        let mmio_req = MmioReq::new(data.to_vec(), len as u8, (bar_base + off as u64), is_write);
        Some(mmio_req)

        // let result = if is_write {
        //     let mut data = self.base.config.config[pci_cfg_data_offset..].as_ref();
        //     self.sys_mem
        //         .write(&mut data, u64(bar_base + off as u64), len as u64)
        // } else {
        //     let mut data = self.base.config.config[pci_cfg_data_offset..].as_mut();
        //     self.sys_mem
        //         .read(&mut data, u64(bar_base + off as u64), len as u64)
        // };
        // if let Err(e) = result {
        //     error!(
        //         "Failed to access virtio configuration through VirtioPciCfgAccessCap. {:?}",
        //         e
        //     );
        // }
    }

    pub fn virtio_pci_auto_queues_num(queues_fixed: u16, nr_cpus: u8, queues_max: usize) -> u16 {
        // Give each vcpu a vq, allow the vCPU that submit request can handle
        // its own request completion. i.e, If the vq is not enough, vcpu A will
        // receive completion of request that submitted by vcpu B, then A needs
        // to IPI B.
        min(queues_max as u16 - queues_fixed, nr_cpus as u16)
    }

    pub fn get_virtio_device(&self) -> &Arc<Mutex<dyn VirtioDevice>> {
        &self.device
    }
}

impl<B: BarAllocTrait + 'static> AsAny for VirtioPciDevice<B> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<B: BarAllocTrait + 'static> PciDevOps<B> for VirtioPciDevice<B> {
    fn name(&self) -> String {
        self.base.id.clone()
    }

    fn pci_base(&self) -> &PciDevBase<B> {
        &self.base
    }

    fn pci_base_mut(&mut self) -> &mut PciDevBase<B> {
        &mut self.base
    }

    fn realize(mut self) -> HyperResult<()> {
        self.init_write_mask(false)?;
        self.init_write_clear_mask(false)?;

        let device_type = self.device.lock().device_type();
        le_write_u16(
            &mut self.base.config.config,
            VENDOR_ID as usize,
            VIRTIO_PCI_VENDOR_ID,
        )?;
        le_write_u16(
            &mut self.base.config.config,
            DEVICE_ID as usize,
            VIRTIO_PCI_DEVICE_ID_BASE + device_type as u16,
        )?;
        self.base.config.config[REVISION_ID] = VIRTIO_PCI_ABI_VERSION;
        let class_id = get_virtio_class_id(device_type);
        le_write_u16(
            &mut self.base.config.config,
            SUB_CLASS_CODE as usize,
            class_id,
        )?;
        le_write_u16(
            &mut self.base.config.config,
            SUBSYSTEM_VENDOR_ID,
            VIRTIO_PCI_VENDOR_ID,
        )?;
        // For compatibility with windows viogpu as front-end drivers.
        let subsysid = if device_type == VIRTIO_TYPE_GPU {
            PCI_SUBDEVICE_ID_QEMU
        } else {
            0x40 + device_type as u16
        };
        le_write_u16(&mut self.base.config.config, SUBSYSTEM_ID, subsysid)?;

        let common_cap = VirtioPciCap::new(
            size_of::<VirtioPciCap>() as u8 + PCI_CAP_VNDR_AND_NEXT_SIZE,
            VirtioPciCapType::Common as u8,
            VIRTIO_PCI_MEM_BAR_IDX,
            VIRTIO_PCI_CAP_COMMON_OFFSET,
            VIRTIO_PCI_CAP_COMMON_LENGTH,
        );
        self.modern_mem_region_cap_add(common_cap)?;

        let isr_cap = VirtioPciCap::new(
            size_of::<VirtioPciCap>() as u8 + PCI_CAP_VNDR_AND_NEXT_SIZE,
            VirtioPciCapType::ISR as u8,
            VIRTIO_PCI_MEM_BAR_IDX,
            VIRTIO_PCI_CAP_ISR_OFFSET,
            VIRTIO_PCI_CAP_ISR_LENGTH,
        );
        self.modern_mem_region_cap_add(isr_cap)?;

        let device_cap = VirtioPciCap::new(
            size_of::<VirtioPciCap>() as u8 + PCI_CAP_VNDR_AND_NEXT_SIZE,
            VirtioPciCapType::Device as u8,
            VIRTIO_PCI_MEM_BAR_IDX,
            VIRTIO_PCI_CAP_DEVICE_OFFSET,
            VIRTIO_PCI_CAP_DEVICE_LENGTH,
        );
        self.modern_mem_region_cap_add(device_cap)?;

        let notify_cap = VirtioPciNotifyCap::new(
            size_of::<VirtioPciNotifyCap>() as u8 + PCI_CAP_VNDR_AND_NEXT_SIZE,
            VirtioPciCapType::Notify as u8,
            VIRTIO_PCI_MEM_BAR_IDX,
            VIRTIO_PCI_CAP_NOTIFY_OFFSET,
            VIRTIO_PCI_CAP_NOTIFY_LENGTH,
            VIRTIO_PCI_CAP_NOTIFY_OFF_MULTIPLIER,
        );
        self.modern_mem_region_cap_add(notify_cap)?;

        let cfg_cap = VirtioPciCfgAccessCap::new(
            size_of::<VirtioPciCfgAccessCap>() as u8 + PCI_CAP_VNDR_AND_NEXT_SIZE,
            VirtioPciCapType::CfgAccess as u8,
        );
        self.cfg_cap_offset = self.modern_mem_region_cap_add(cfg_cap)?;

        // Make related fields of PCI config writable for VirtioPciCfgAccessCap.
        let write_mask = &mut self.base.config.write_mask[self.cfg_cap_offset..];
        write_mask[offset_of!(VirtioPciCap, bar_id)] = !0;
        le_write_u32(write_mask, offset_of!(VirtioPciCap, offset), !0)?;
        le_write_u32(write_mask, offset_of!(VirtioPciCap, length), !0)?;
        le_write_u32(
            write_mask,
            offset_of!(VirtioPciCfgAccessCap, pci_cfg_data),
            !0,
        )?;

        let nvectors = self.device.lock().queue_num() + 1;
        init_msix(
            &mut self.base,
            VIRTIO_PCI_MSIX_BAR_IDX as usize,
            nvectors as u32,
            self.dev_id.clone(),
            None,
        )?;

        self.assign_interrupt_cb();

        self.device.lock().realize().or_else(|_| {
            Err(HyperError::VirtioError(VirtioError::Other(format!(
                "Failed to realize virtio device"
            ))))
        })?;

        let name = self.name();
        let devfn = self.base.devfn;
        let dev = Arc::new(Mutex::new(self));
        let mut mem_region_size = ((VIRTIO_PCI_CAP_NOTIFY_OFFSET + VIRTIO_PCI_CAP_NOTIFY_LENGTH)
            as u64)
            .next_power_of_two();
        mem_region_size = max(mem_region_size, MINIMUM_BAR_SIZE_FOR_MMIO as u64);
        let pci_cfg_cap_ops = Self::build_pci_cfg_cap_ops(dev.clone());

        dev.lock().base.config.register_bar(
            VIRTIO_PCI_MEM_BAR_IDX as usize,
            Some(pci_cfg_cap_ops),
            RegionType::Mem64Bit,
            false,
            mem_region_size,
        )?;

        // Register device to pci bus. Now set it to the root bus.
        let pci_bus = dev.lock().base.parent_bus.upgrade().unwrap();
        let mut locked_pci_bus = pci_bus.lock();
        let pci_device = locked_pci_bus.devices.get(&devfn);
        if pci_device.is_none() {
            locked_pci_bus.devices.insert(devfn, dev.clone());
        } else {
            error!(
                "Devfn {:?} has been used by {:?}",
                &devfn,
                pci_device.unwrap().lock().name()
            );
        }

        Ok(())
    }

    fn unrealize(&mut self) -> HyperResult<()> {
        self.device.lock().unrealize().or_else(|_| {
            Err(HyperError::VirtioError(VirtioError::Other(format!(
                "Failed to unrealize the virtio device"
            ))))
        })?;

        let bus = self.base.parent_bus.upgrade().unwrap();
        self.base.config.unregister_bars(&bus)?;

        Ok(())
    }

    fn read_config(&mut self, offset: usize, data: &mut [u8]) {
        debug!(
            "Read pci config space at offset {:#x} with data size {}",
            offset,
            data.len()
        );
        let mmio_req = self.do_cfg_access(offset, offset + data.len(), false);
        if mmio_req.is_some() {
            *GLOBAL_VIRTIO_PCI_CFG_REQ.write() = mmio_req;
            return;
        }
        self.base.config.read(offset, data);
    }

    fn write_config(&mut self, offset: usize, data: &[u8]) {
        debug!(
            "Write pci config space at offset {:#x} with data size {}",
            offset,
            data.len()
        );
        let data_size = data.len();
        let end = offset + data_size;
        if end > PCIE_CONFIG_SPACE_SIZE || data_size > REG_SIZE {
            error!(
                "Failed to write pcie config space at offset 0x{:x} with data size {}",
                offset, data_size
            );
            return;
        }

        let parent_bus = self.base.parent_bus.upgrade().unwrap();
        let locked_parent_bus = parent_bus.lock();
        self.base
            .config
            .write(offset, data, self.dev_id.clone().load(Ordering::Acquire));
        let mmio_req = self.do_cfg_access(offset, end, true);
        if mmio_req.is_some() {
            *GLOBAL_VIRTIO_PCI_CFG_REQ.write() = mmio_req;
        }
    }

    fn reset(&mut self, _reset_child_device: bool) -> HyperResult<()> {
        self.deactivate_device();
        self.device.lock().reset().or_else(|_| {
            Err(HyperError::VirtioError(VirtioError::Other(format!(
                "Failed to reset virtio device"
            ))))
        })?;
        self.base.config.reset()?;

        Ok(())
    }

    fn get_dev_path(&self) -> Option<String> {
        let parent_bus = self.base.parent_bus.upgrade().unwrap();
        match self.device.lock().device_type() {
            VIRTIO_TYPE_BLOCK => {
                // The virtio blk device is identified as a single-channel SCSI device,
                // so add scsi controller identification without channel, scsi-id and lun.
                let parent_dev_path = self.get_parent_dev_path(parent_bus);
                let mut dev_path =
                    self.populate_dev_path(parent_dev_path, self.base.devfn, "/scsi@");
                dev_path.push_str("/disk@0,0");
                Some(dev_path)
            }
            VIRTIO_TYPE_SCSI => {
                // The virtio scsi controller can not set boot order, which is set for scsi device.
                // All the scsi devices in the same scsi controller have the same boot path prefix
                // (eg: /pci@XXXXX/scsi@$slot_id[,function_id]). And every scsi device has it's
                // own boot path("/channel@0/disk@$target_id,$lun_id");
                let parent_dev_path = self.get_parent_dev_path(parent_bus);
                let dev_path = self.populate_dev_path(parent_dev_path, self.base.devfn, "/scsi@");
                Some(dev_path)
            }
            _ => None,
        }
    }
}
