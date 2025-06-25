//! [ArceOS](https://github.com/arceos-org/arceos) filesystem module.
//!
//! It provides unified filesystem operations for various filesystems.
//!
//! # Cargo Features
//!
//! - `fatfs`: Use [FAT] as the main filesystem and mount it on `/`. This feature
//!    is **enabled** by default.
//! - `devfs`: Mount [`axfs_devfs::DeviceFileSystem`] on `/dev`. This feature is
//!    **enabled** by default.
//! - `ramfs`: Mount [`axfs_ramfs::RamFileSystem`] on `/tmp`. This feature is
//!    **enabled** by default.
//! - `myfs`: Allow users to define their custom filesystems to override the
//!    default. In this case, [`MyFileSystemIf`] is required to be implemented
//!    to create and initialize other filesystems. This feature is **disabled** by
//!    by default, but it will override other filesystem selection features if
//!    both are enabled.
//!
//! [FAT]: https://en.wikipedia.org/wiki/File_Allocation_Table
//! [`MyFileSystemIf`]: fops::MyFileSystemIf

#![cfg_attr(all(not(test), not(doc)), no_std)]
#![feature(doc_auto_cfg)]

#[macro_use]
extern crate log;
extern crate alloc;

mod dev;
mod fs;
mod mounts;
mod root;

pub mod api;
pub mod fops;

use axdriver::{AxDeviceContainer, prelude::*};

use alloc::rc::Rc;
use core::cell::RefCell;
pub use partman::BootArgsFileSystem;
use partman::{PartBlock, PartManError, PartManager};

/// Initializes filesystems by block devices.
pub fn init_filesystems(
    mut blk_devs: AxDeviceContainer<AxBlockDevice>,
    rootargs: BootArgsFileSystem,
) {
    info!("Initialize filesystems...");
    let dev: AxBlockDevice = blk_devs.take_one().expect("No block device found!");

    info!("  use block device 0: {:?}", dev.device_name());

    let dev_rc = Rc::new(RefCell::new(dev));

    let part_offset = {
        let mut block_device_wrapper = BlockDeviceWrapper::new(dev_rc.clone());
        match PartManager::new(&mut block_device_wrapper, rootargs) {
            Ok(mut partman) => partman.part_offset().unwrap_or(0),
            Err(_) => 0,
        }
    };

    let dev = Rc::try_unwrap(dev_rc)
        .map_err(|_| "Failed to unwrap device")
        .unwrap()
        .into_inner();

    self::root::init_rootfs(self::dev::Disk::new(dev), part_offset);
}

pub struct BlockDeviceWrapper {
    pub inner: Rc<RefCell<AxBlockDevice>>,
}

impl BlockDeviceWrapper {
    pub fn new(device: Rc<RefCell<AxBlockDevice>>) -> Self {
        Self { inner: device }
    }
}

impl PartBlock for BlockDeviceWrapper {
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), PartManError> {
        self.inner
            .borrow_mut()
            .read_block(block_id, buf)
            .map_err(|_| PartManError::InvalidData)
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), PartManError> {
        self.inner
            .borrow_mut()
            .write_block(block_id, buf)
            .map_err(|_| PartManError::InvalidData)
    }

    fn block_size(&self) -> usize {
        self.inner.borrow().block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.inner.borrow().num_blocks()
    }
}
