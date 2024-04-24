use super::{
    checked_offset_mem, ElemIovec, Element, VringOps, INVALID_VECTOR_NUM, VIRTQ_DESC_F_INDIRECT,
    VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE,
};
use crate::device::virtio::{
    report_virtio_error, virtio_has_feature, VirtioInterrupt, VIRTIO_F_RING_EVENT_IDX,
};
use alloc::format;
use alloc::sync::Arc;
use core::cmp::{max, min, Ordering};
use core::mem::size_of;
use core::num::Wrapping;
use core::ops::Deref;
use core::ops::DerefMut;
use core::sync::atomic::AtomicBool;
use hypercraft::{HyperError, HyperResult as Result, VirtioError};
use pci::util::byte_code::ByteCode;

/// When host consumes a buffer, don't interrupt the guest.
const VRING_AVAIL_F_NO_INTERRUPT: u16 = 1;
/// When guest produces a buffer, don't notify the host.
const VRING_USED_F_NO_NOTIFY: u16 = 1;

/// Max total len of a descriptor chain.
const DESC_CHAIN_MAX_TOTAL_LEN: u64 = 1u64 << 32;
/// The length of used element.
const USEDELEM_LEN: u64 = size_of::<UsedElem>() as u64;
/// The length of avail element.
const AVAILELEM_LEN: u64 = size_of::<u16>() as u64;
/// The length of available ring except array of avail element(flags: u16 idx: u16 used_event: u16).
const VRING_AVAIL_LEN_EXCEPT_AVAILELEM: u64 = (size_of::<u16>() * 3) as u64;
/// The length of used ring except array of used element(flags: u16 idx: u16 avail_event: u16).
const VRING_USED_LEN_EXCEPT_USEDELEM: u64 = (size_of::<u16>() * 3) as u64;
/// The length of flags(u16) and idx(u16).
const VRING_FLAGS_AND_IDX_LEN: u64 = size_of::<SplitVringFlagsIdx>() as u64;
/// The position of idx in the available ring and the used ring.
const VRING_IDX_POSITION: u64 = size_of::<u16>() as u64;
/// The length of virtio descriptor.
const DESCRIPTOR_LEN: u64 = size_of::<SplitVringDesc>() as u64;

#[derive(Default, Clone, Copy)]
pub struct VirtioAddrCache {
    /// Host virtual address of the descriptor table.
    pub desc_table_host: u64,
    /// Host virtual address of the available ring.
    pub avail_ring_host: u64,
    /// Host virtual address of the used ring.
    pub used_ring_host: u64,
}

/// The configuration of virtqueue.
#[derive(Default, Clone, Copy)]
pub struct QueueConfig {
    /// Guest physical address of the descriptor table.
    pub desc_table: u64,
    /// Guest physical address of the available ring.
    pub avail_ring: u64,
    /// Guest physical address of the used ring.
    pub used_ring: u64,
    /// Host address cache.
    pub addr_cache: VirtioAddrCache,
    /// The maximal size of elements offered by the device.
    pub max_size: u16,
    /// The queue size set by the guest.
    pub size: u16,
    /// Virtual queue ready bit.
    pub ready: bool,
    /// Interrupt vector index of the queue for msix
    pub vector: u16,
    /// The next index which can be popped in the available vring.
    next_avail: Wrapping<u16>,
    /// The next index which can be pushed in the used vring.
    next_used: Wrapping<u16>,
    /// The index of last descriptor used which has triggered interrupt.
    last_signal_used: Wrapping<u16>,
    /// The last_signal_used is valid or not.
    signal_used_valid: bool,
}

impl QueueConfig {
    /// Create configuration for a virtqueue.
    ///
    /// # Arguments
    ///
    /// * `max_size` - The maximum size of the virtqueue.
    pub fn new(max_size: u16) -> Self {
        let addr_cache = VirtioAddrCache::default();
        QueueConfig {
            desc_table: 0,
            avail_ring: 0,
            used_ring: 0,
            addr_cache,
            max_size,
            size: max_size,
            ready: false,
            vector: INVALID_VECTOR_NUM,
            next_avail: Wrapping(0),
            next_used: Wrapping(0),
            last_signal_used: Wrapping(0),
            signal_used_valid: false,
        }
    }

    fn get_desc_size(&self) -> u64 {
        min(self.size, self.max_size) as u64 * DESCRIPTOR_LEN
    }

    fn get_used_size(&self, features: u64) -> u64 {
        let size = if virtio_has_feature(features, VIRTIO_F_RING_EVENT_IDX) {
            2_u64
        } else {
            0_u64
        };

        size + VRING_FLAGS_AND_IDX_LEN + (min(self.size, self.max_size) as u64) * USEDELEM_LEN
    }

    fn get_avail_size(&self, features: u64) -> u64 {
        let size = if virtio_has_feature(features, VIRTIO_F_RING_EVENT_IDX) {
            2_u64
        } else {
            0_u64
        };

        size + VRING_FLAGS_AND_IDX_LEN
            + (min(self.size, self.max_size) as u64) * (size_of::<u16>() as u64)
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.max_size);
    }

    pub fn set_addr_cache(
        &mut self,
        interrupt_cb: Arc<VirtioInterrupt>,
        features: u64,
        broken: &Arc<AtomicBool>,
    ) {
    }
}

/// Virtio used element.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UsedElem {
    /// Index of descriptor in the virqueue descriptor table.
    id: u32,
    /// Total length of the descriptor chain which was used (written to).
    len: u32,
}

impl ByteCode for UsedElem {}

/// A struct including flags and idx for avail vring and used vring.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SplitVringFlagsIdx {
    flags: u16,
    idx: u16,
}

impl ByteCode for SplitVringFlagsIdx {}

struct DescInfo {
    /// The host virtual address of the descriptor table.
    table_host: u64,
    /// The size of the descriptor table.
    size: u16,
    /// The index of the current descriptor table.
    index: u16,
    /// The descriptor table.
    desc: SplitVringDesc,
}

/// Descriptor of split vring.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SplitVringDesc {
    /// Address (guest-physical).
    pub addr: u64,
    /// Length.
    pub len: u32,
    /// The flags as indicated above.
    pub flags: u16,
    /// We chain unused descriptors via this, too.
    pub next: u16,
}

impl SplitVringDesc {
    /// Create a descriptor of split vring.
    ///
    /// # Arguments
    ///
    /// * `desc_table` - Guest address of virtqueue descriptor table.
    /// * `queue_size` - Size of virtqueue.
    /// * `index` - Index of descriptor in the virqueue descriptor table.
    fn new(desc_table_host: u64, queue_size: u16, index: u16) -> Result<Self> {
        let desc_addr = desc_table_host
            .checked_add(u64::from(index) * DESCRIPTOR_LEN)
            .ok_or_else(|| {
                HyperError::VirtioError(VirtioError::AddressOverflow(
                    "creating a descriptor",
                    desc_table_host,
                    u64::from(index) * DESCRIPTOR_LEN,
                ))
            })?;
        Ok(SplitVringDesc {
            addr: desc_addr,
            len: 0,
            flags: 0,
            next: 0,
        })
    }

    /// Return true if the descriptor is valid.
    fn is_valid(&self, queue_size: u16) -> bool {
        true
    }

    /// Return true if this descriptor has next descriptor.
    fn has_next(&self) -> bool {
        self.flags & VIRTQ_DESC_F_NEXT != 0
    }

    /// Get the next descriptor in descriptor chain.
    fn next_desc(desc_table_host: u64, queue_size: u16, index: u16) -> Result<SplitVringDesc> {
        SplitVringDesc::new(desc_table_host, queue_size, index)
    }

    /// Check whether this descriptor is write-only or read-only.
    /// Write-only means that the emulated device can write and the driver can read.
    fn write_only(&self) -> bool {
        self.flags & VIRTQ_DESC_F_WRITE != 0
    }

    /// Return true if this descriptor is a indirect descriptor.
    fn is_indirect_desc(&self) -> bool {
        self.flags & VIRTQ_DESC_F_INDIRECT != 0
    }

    /// Return true if the indirect descriptor is valid.
    /// The len can be divided evenly by the size of descriptor and can not be zero.
    fn is_valid_indirect_desc(&self) -> bool {
        if self.len == 0
            || u64::from(self.len) % DESCRIPTOR_LEN != 0
            || u64::from(self.len) / DESCRIPTOR_LEN > u16::MAX as u64
        {
            error!("The indirect descriptor is invalid, len: {}", self.len);
            return false;
        }
        if self.has_next() {
            error!("INDIRECT and NEXT flag should not be used together");
            return false;
        }
        true
    }

    /// Get the num of descriptor in the table of indirect descriptor.
    fn get_desc_num(&self) -> u16 {
        (u64::from(self.len) / DESCRIPTOR_LEN) as u16
    }

    /// Get element from descriptor chain.
    fn get_element(desc_info: &DescInfo, elem: &mut Element) -> Result<()> {
        let mut desc_table_host = desc_info.table_host;
        let mut desc_size = desc_info.size;
        let mut desc = desc_info.desc;
        elem.index = desc_info.index;
        let mut queue_size = desc_size;
        let mut indirect: bool = false;
        let mut write_elem_count: u32 = 0;
        let mut desc_total_len: u64 = 0;

        Ok(())
    }
}

impl ByteCode for SplitVringDesc {}

/// Split vring.
#[derive(Default, Clone, Copy)]
pub struct SplitVring {
    /// The configuration of virtqueue.
    queue_config: QueueConfig,
}

impl Deref for SplitVring {
    type Target = QueueConfig;
    fn deref(&self) -> &Self::Target {
        &self.queue_config
    }
}

impl DerefMut for SplitVring {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.queue_config
    }
}

impl SplitVring {
    /// Create a split vring.
    ///
    /// # Arguments
    ///
    /// * `queue_config` - Configuration of the vring.
    pub fn new(queue_config: QueueConfig) -> Self {
        SplitVring { queue_config }
    }

    /// The actual size of the queue.
    fn actual_size(&self) -> u16 {
        min(self.size, self.max_size)
    }

    /// Get the flags and idx of the available ring from guest memory.
    fn get_avail_flags_idx(&self) -> Result<SplitVringFlagsIdx> {
        Ok(SplitVringFlagsIdx { flags: 0, idx: 0 })
    }

    /// Get the idx of the available ring from guest memory.
    fn get_avail_idx(&self) -> Result<u16> {
        let flags_idx = self.get_avail_flags_idx()?;
        Ok(flags_idx.idx)
    }

    /// Get the flags of the available ring from guest memory.
    fn get_avail_flags(&self) -> Result<u16> {
        let flags_idx = self.get_avail_flags_idx()?;
        Ok(flags_idx.flags)
    }

    /// Get the flags and idx of the used ring from guest memory.
    fn get_used_flags_idx(&self) -> Result<SplitVringFlagsIdx> {
        Ok(SplitVringFlagsIdx { flags: 0, idx: 0 })
    }

    /// Get the index of the used ring from guest memory.
    fn get_used_idx(&self) -> Result<u16> {
        let flag_idx = self.get_used_flags_idx()?;
        Ok(flag_idx.idx)
    }

    /// Set the used flags to suppress virtqueue notification or not
    fn set_used_flags(&self, suppress: bool) -> Result<()> {
        let mut flags_idx = self.get_used_flags_idx()?;

        if suppress {
            flags_idx.flags |= VRING_USED_F_NO_NOTIFY;
        } else {
            flags_idx.flags &= !VRING_USED_F_NO_NOTIFY;
        }
        Ok(())
    }

    /// Set the avail idx to the field of the event index for the available ring.
    fn set_avail_event(&self, event_idx: u16) -> Result<()> {
        let avail_event_offset =
            VRING_FLAGS_AND_IDX_LEN + USEDELEM_LEN * u64::from(self.actual_size());
        Ok(())
    }

    /// Get the event index of the used ring from guest memory.
    fn get_used_event(&self) -> Result<u16> {
        Ok(0)
    }

    /// Return true if VRING_AVAIL_F_NO_INTERRUPT is set.
    fn is_avail_ring_no_interrupt(&self) -> bool {
        true
    }

    /// Return true if it's required to trigger interrupt for the used vring.
    fn used_ring_need_event(&mut self) -> bool {
        true
    }

    fn is_overlap(start1: u64, end1: u64, start2: u64, end2: u64) -> bool {
        !(start1 >= end2 || start2 >= end1)
    }

    fn is_invalid_memory(&self, actual_size: u64) -> bool {
        true
    }

    fn get_desc_info(&mut self, next_avail: Wrapping<u16>, features: u64) -> Result<DescInfo> {
        let index_offset =
            VRING_FLAGS_AND_IDX_LEN + AVAILELEM_LEN * u64::from(next_avail.0 % self.actual_size());
        // The GPA of avail_ring_host with avail table length has been checked in
        // is_invalid_memory which must not be overflowed.
        let desc_index_addr = self.addr_cache.avail_ring_host + index_offset;
        let desc_index = 0;

        let desc = SplitVringDesc::new(
            self.addr_cache.desc_table_host,
            self.actual_size(),
            desc_index,
        )?;

        // Suppress queue notification related to current processing desc chain.
        if virtio_has_feature(features, VIRTIO_F_RING_EVENT_IDX) {
            self.set_avail_event((next_avail + Wrapping(1)).0)
                .or_else(|_| {
                    Err(HyperError::VirtioError(VirtioError::Other(format!(
                        "Failed to set avail event for popping avail ring"
                    ))))
                })?;
        }

        Ok(DescInfo {
            table_host: self.addr_cache.desc_table_host,
            size: self.actual_size(),
            index: desc_index,
            desc,
        })
    }

    fn get_vring_element(&mut self, features: u64, elem: &mut Element) -> Result<()> {
        let desc_info = self.get_desc_info(self.next_avail, features)?;

        SplitVringDesc::get_element(&desc_info, elem).or_else(|_| {
            Err(HyperError::VirtioError(VirtioError::Other(format!(
                "Failed to get element from descriptor chain {}, table addr: 0x{:X}, size: {}",
                desc_info.index, desc_info.table_host, desc_info.size,
            ))))
        })?;
        self.next_avail += Wrapping(1);

        Ok(())
    }
}

impl VringOps for SplitVring {
    fn is_enabled(&self) -> bool {
        self.ready
    }

    fn is_valid(&self) -> bool {
        let size = u64::from(self.actual_size());
        if !self.ready {
            error!("The configuration of vring is not ready\n");
            false
        } else if self.size > self.max_size || self.size == 0 || (self.size & (self.size - 1)) != 0
        {
            error!(
                "vring with invalid size:{} max size:{}",
                self.size, self.max_size
            );
            false
        } else {
            !self.is_invalid_memory(size)
        }
    }

    fn pop_avail(&mut self, features: u64) -> Result<Element> {
        let mut element = Element::new(0);

        Ok(element)
    }

    fn push_back(&mut self) {
        self.next_avail -= Wrapping(1);
    }

    fn add_used(&mut self, index: u16, len: u32) -> Result<()> {
        Ok(())
    }

    fn should_notify(&mut self, features: u64) -> bool {
        true
    }

    fn suppress_queue_notify(&mut self, features: u64, suppress: bool) -> Result<()> {
        Ok(())
    }

    fn actual_size(&self) -> u16 {
        self.actual_size()
    }

    fn get_queue_config(&self) -> QueueConfig {
        let mut config = self.queue_config;
        config.signal_used_valid = false;
        config
    }

    /// The number of descriptor chains in the available ring.
    fn avail_ring_len(&mut self) -> Result<u16> {
        let avail_idx = self.get_avail_idx().map(Wrapping)?;

        Ok((avail_idx - self.next_avail).0)
    }

    fn get_avail_idx(&self) -> Result<u16> {
        SplitVring::get_avail_idx(self)
    }

    fn get_used_idx(&self) -> Result<u16> {
        SplitVring::get_used_idx(self)
    }

    fn get_cache(&self) -> &Option<u32> {
        &None
    }

    fn get_avail_bytes(&mut self, max_size: usize, is_in: bool) -> Result<usize> {
        Ok(0)
    }
}
