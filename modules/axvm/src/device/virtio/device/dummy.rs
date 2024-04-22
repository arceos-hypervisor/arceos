use crate::device::virtio::{VirtioBase, VirtioDevice};
use hypercraft::{HyperError, HyperResult as Result};
use pci::{le_read_u32, le_write_u32};
use core::any::Any;
use core::sync::atomic::Ordering;
use hypercraft::VirtioError;
use alloc::format;
use alloc::sync::Arc;
use pci::AsAny;

use crate::device::virtio::{
    QueueConfig, VirtioInterrupt, 
};
use crate::device::virtio::{
    CONFIG_STATUS_ACKNOWLEDGE, CONFIG_STATUS_DRIVER, CONFIG_STATUS_DRIVER_OK, CONFIG_STATUS_FAILED,
    CONFIG_STATUS_FEATURES_OK, CONFIG_STATUS_NEEDS_RESET, INVALID_VECTOR_NUM,
    QUEUE_TYPE_PACKED_VRING, QUEUE_TYPE_SPLIT_VRING, VIRTIO_F_RING_PACKED, VIRTIO_F_VERSION_1,
    VIRTIO_MMIO_INT_CONFIG, VIRTIO_MMIO_INT_VRING, VIRTIO_TYPE_BLOCK, VIRTIO_TYPE_CONSOLE,
    VIRTIO_TYPE_FS, VIRTIO_TYPE_GPU, VIRTIO_TYPE_NET, VIRTIO_TYPE_SCSI,
};

pub struct DummyVirtioDevice {
    pub base: VirtioBase,
}

impl DummyVirtioDevice {
    pub fn new(device_type: u32, queue_num: usize, queue_size_max: u16) -> Self {
        Self {
            base: VirtioBase::new(device_type, queue_num, queue_size_max),
        }
    }
}

impl AsAny for DummyVirtioDevice {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}


impl VirtioDevice for DummyVirtioDevice {
    /// Get base property of virtio device.
    fn virtio_base(&self) -> &VirtioBase {
        &self.base
    }

    /// Get mutable base property virtio device.
    fn virtio_base_mut(&mut self) -> &mut VirtioBase {
        &mut self.base
    }

    /// Realize low level device.
    fn realize(&mut self) -> Result<()> {
        Ok(())
    }

    /// Unrealize low level device.
    fn unrealize(&mut self) -> Result<()> {
        error!("Unrealize of the virtio device is not implemented");
        Err(HyperError::BadState)
    }

    /// Get the virtio device type, refer to Virtio Spec.
    fn device_type(&self) -> u32 {
        self.virtio_base().device_type
    }

    /// Get the count of virtio device queues.
    fn queue_num(&self) -> usize {
        self.virtio_base().queue_num
    }

    /// Get the queue size of virtio device.
    fn queue_size_max(&self) -> u16 {
        self.virtio_base().queue_size_max
    }

    /// Init device configure space and features.
    fn init_config_features(&mut self) -> Result<()>{
        Ok(())
    }

    /// Get device features from host.
    fn device_features(&self, features_select: u32) -> u32 {
        let buf = self.virtio_base().device_features.to_le_bytes();
        le_read_u32(&buf[..], features_select as usize).unwrap_or(0)
    }

    /// Set driver features by guest.
    fn set_driver_features(&mut self, page: u32, value: u32) {
        let mut v = value;
        let unsupported_features = value & !self.device_features(page);
        if unsupported_features != 0 {
            warn!(
                "Receive acknowledge request with unknown feature",
            );
            v &= !unsupported_features;
        }

        let features = if page == 0 {
            (self.driver_features(1) as u64) << 32 | (v as u64)
        } else {
            (v as u64) << 32 | (self.driver_features(0) as u64)
        };
        self.virtio_base_mut().driver_features = features;
    }

    /// Get driver features by guest.
    fn driver_features(&self, features_select: u32) -> u32 {
        let buf = self.virtio_base().driver_features.to_le_bytes();
        le_read_u32(&buf[..], features_select as usize).unwrap_or(0)
    }

    /// Get host feature selector.
    fn hfeatures_sel(&self) -> u32 {
        self.virtio_base().hfeatures_sel
    }

    /// Set host feature selector.
    fn set_hfeatures_sel(&mut self, val: u32) {
        self.virtio_base_mut().hfeatures_sel = val;
    }

    /// Get guest feature selector.
    fn gfeatures_sel(&self) -> u32 {
        self.virtio_base().gfeatures_sel
    }

    /// Set guest feature selector.
    fn set_gfeatures_sel(&mut self, val: u32) {
        self.virtio_base_mut().gfeatures_sel = val;
    }

    /// Check whether virtio device status is as expected.
    fn check_device_status(&self, set: u32, clr: u32) -> bool {
        self.device_status() & (set | clr) == set
    }

    /// Get the status of virtio device.
    fn device_status(&self) -> u32 {
        self.virtio_base().device_status.load(Ordering::Acquire)
    }

    /// Set the status of virtio device.
    fn set_device_status(&mut self, val: u32) {
        self.virtio_base_mut()
            .device_status
            .store(val, Ordering::SeqCst)
    }

    /// Check device is activated or not.
    fn device_activated(&self) -> bool {
        self.virtio_base().device_activated.load(Ordering::Acquire)
    }

    /// Set device activate status.
    fn set_device_activated(&mut self, val: bool) {
        self.virtio_base_mut()
            .device_activated
            .store(val, Ordering::SeqCst)
    }

    /// Get config generation.
    fn config_generation(&self) -> u8 {
        self.virtio_base().config_generation.load(Ordering::Acquire)
    }

    /// Set config generation.
    fn set_config_generation(&mut self, val: u8) {
        self.virtio_base_mut()
            .config_generation
            .store(val, Ordering::SeqCst);
    }

    /// Get msix vector of config change interrupt.
    fn config_vector(&self) -> u16 {
        self.virtio_base().config_vector.load(Ordering::Acquire)
    }

    /// Set msix vector of config change interrupt.
    fn set_config_vector(&mut self, val: u16) {
        self.virtio_base_mut()
            .config_vector
            .store(val, Ordering::SeqCst);
    }

    /// Get virtqueue type.
    fn queue_type(&self) -> u16 {
        self.virtio_base().queue_type
    }

    /// Set virtqueue type.
    fn set_queue_type(&mut self, val: u16) {
        self.virtio_base_mut().queue_type = val;
    }

    /// Get virtqueue selector.
    fn queue_select(&self) -> u16 {
        self.virtio_base().queue_select
    }

    /// Set virtqueue selector.
    fn set_queue_select(&mut self, val: u16) {
        self.virtio_base_mut().queue_select = val;
    }

    /// Get virtqueue config.
    fn queue_config(&self) -> Result<&QueueConfig> {
        let queues_config = &self.virtio_base().queues_config;
        let queue_select = self.virtio_base().queue_select;
        queues_config
            .get(queue_select as usize)
            .ok_or_else(|| HyperError::VirtioError(VirtioError::Other(format!("queue_select overflows"))))
    }

    /// Get mutable virtqueue config.
    fn queue_config_mut(&mut self, need_check: bool) -> Result<&mut QueueConfig> {
        if need_check
            && !self.check_device_status(
                CONFIG_STATUS_FEATURES_OK,
                CONFIG_STATUS_DRIVER_OK | CONFIG_STATUS_FAILED,
            )
        {
            return Err(HyperError::VirtioError(VirtioError::DevStatErr(self.device_status())));
        }

        let queue_select = self.virtio_base().queue_select;
        let queues_config = &mut self.virtio_base_mut().queues_config;
        return queues_config
            .get_mut(queue_select as usize)
            .ok_or_else(|| HyperError::VirtioError(VirtioError::Other(format!("queue_select overflows"))));
    }

    /// Get ISR register.
    fn interrupt_status(&self) -> u32 {
        self.virtio_base().interrupt_status.load(Ordering::Acquire)
    }

    /// Set ISR register.
    fn set_interrupt_status(&mut self, val: u32) {
        self.virtio_base_mut()
            .interrupt_status
            .store(val, Ordering::SeqCst)
    }

    /// Read data of config from guest.
    fn read_config(&self, offset: u64, data: &mut [u8]) -> Result<()> {
        Ok(())
    }

    /// Write data to config from guest.
    fn write_config(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        Ok(())
    }

    /// Activate the virtio device, this function is called by vcpu thread when frontend
    /// virtio driver is ready and write `DRIVER_OK` to backend.
    ///
    /// # Arguments
    ///
    /// * `mem_space` - System mem.
    /// * `interrupt_cb` - The callback used to send interrupt to guest.
    /// * `queues` - The virtio queues.
    /// * `queue_evts` - The notifier events from guest.
    fn activate(
        &mut self,
        interrupt_cb: Arc<VirtioInterrupt>,
    ) -> Result<()> {
        Ok(())
    }

    /// Deactivate virtio device, this function remove event fd
    /// of device out of the event loop.
    fn deactivate(&mut self) -> Result<()> {
        error!(
            "Reset this device is not supported, virtio dev type is {}",
            self.device_type()
        );
        Err(HyperError::BadState)
    }

    /// Reset virtio device, used to do some special reset action for
    /// different device.
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    /// Update the low level config of MMIO device,
    /// for example: update the images file fd of virtio block device.
    ///
    /// # Arguments
    ///
    /// * `_file_path` - The related backend file path.
    fn update_config(&mut self) -> Result<()> {
        error!("Unsupported to update configuration");
        Err(HyperError::BadState)
    }

    /// Get whether the virtio device has a control queue,
    /// devices with a control queue should override this function.
    fn has_control_queue(&self) -> bool {
        false
    }
}