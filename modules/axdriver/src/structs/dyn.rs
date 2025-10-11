#![allow(unused_imports)]

use alloc::{boxed::Box, vec, vec::Vec};
use core::{ops::Deref, ptr::NonNull};

use rdrive::register::{DriverRegister, DriverRegisterSlice};

#[allow(unused_imports)]
use crate::prelude::*;

/// The unified type of the NIC devices.
#[cfg(feature = "net")]
pub type AxNetDevice = Box<dyn NetDriverOps>;
/// The unified type of the block storage devices.
#[cfg(feature = "block")]
pub type AxBlockDevice = Box<dyn BlockDriverOps>;
/// The unified type of the graphics display devices.
#[cfg(feature = "display")]
pub type AxDisplayDevice = Box<dyn DisplayDriverOps>;
/// The unified type of the input devices.
#[cfg(feature = "input")]
pub type AxInputDevice = Box<dyn InputDriverOps>;

impl super::AxDeviceEnum {
    /// Constructs a network device.
    #[cfg(feature = "net")]
    pub fn from_net(dev: impl NetDriverOps + 'static) -> Self {
        Self::Net(Box::new(dev))
    }

    /// Constructs a block device.
    #[cfg(feature = "block")]
    pub fn from_block(dev: impl BlockDriverOps + 'static) -> Self {
        Self::Block(Box::new(dev))
    }

    /// Constructs a display device.
    #[cfg(feature = "display")]
    pub fn from_display(dev: impl DisplayDriverOps + 'static) -> Self {
        Self::Display(Box::new(dev))
    }

    /// Constructs an input device.
    #[cfg(feature = "input")]
    pub fn from_input(dev: impl InputDriverOps + 'static) -> Self {
        Self::Input(Box::new(dev))
    }
}

pub fn probe_all_devices() -> Vec<super::AxDeviceEnum> {
    rdrive::probe_all(true).unwrap();
    #[allow(unused_mut)]
    let mut devices = Vec::new();
    #[cfg(feature = "block")]
    {
        use axdriver_block::gpt::GptPartitionDev;
        let ls = rdrive::get_list::<rdif_block::Block>();
        for dev in ls {
            let mut dev_blk = crate::dyn_drivers::blk::Block::from(dev);
            if dev_blk.is_gpt_partition() {
                let root = "rootfs".parse().unwrap();
                match GptPartitionDev::try_new(dev_blk, |_, part| part.name == root) {
                    Ok(d) => {
                        info!("Found block device with GPT partition: {}", d.device_name());
                        devices.push(super::AxDeviceEnum::from_block(d));
                    }
                    Err(e) => {
                        info!("Failed to find GPT partition 'root': {e}");
                        // devices.push(super::AxDeviceEnum::from_block(dev_blk));
                    }
                }
            } else {
                devices.push(super::AxDeviceEnum::from_block(dev_blk));
            }
        }
    }
    devices
}
