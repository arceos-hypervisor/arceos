//! [ArceOS](https://github.com/arceos-org/arceos) filesystem module.
//!
//! It provides unified filesystem operations for various filesystems.
//!
//! # Cargo Features
//!
//! - `fatfs`: Use [FAT] as the main filesystem and mount it on `/`. This feature
//!   is **enabled** by default.
//! - `devfs`: Mount [`axfs_devfs::DeviceFileSystem`] on `/dev`. This feature is
//!   **enabled** by default.
//! - `ramfs`: Mount [`axfs_ramfs::RamFileSystem`] on `/tmp`. This feature is
//!   **enabled** by default.
//! - `myfs`: Allow users to define their custom filesystems to override the
//!   default. In this case, [`MyFileSystemIf`] is required to be implemented
//!   to create and initialize other filesystems. This feature is **disabled** by
//!   by default, but it will override other filesystem selection features if
//!   both are enabled.
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
mod partition;
mod root;

pub mod api;
pub mod fops;

use alloc::sync::Arc;
use axdriver::{AxDeviceContainer, prelude::*};

/// Initializes filesystems by block devices.
pub fn init_filesystems(mut blk_devs: AxDeviceContainer<AxBlockDevice>) {
    info!("Initialize filesystems...");

    let dev = blk_devs.take_one().expect("No block device found!");
    info!("  use block device 0: {:?}", dev.device_name());
    let mut disk = self::dev::Disk::new(dev);

    // Try to scan GPT partitions first
    match self::partition::scan_gpt_partitions(&mut disk) {
        Ok(partitions) if !partitions.is_empty() => {
            info!(
                "Found {} partitions, initializing with dynamic filesystem detection",
                partitions.len()
            );
            // Check if any partition has a supported filesystem
            let has_supported_fs = partitions.iter().any(|p| p.filesystem_type.is_some());
            if has_supported_fs {
                // Try to initialize with partitions
                let disk_arc = Arc::new(disk);
                if !self::root::init_rootfs_with_partitions(disk_arc, partitions) {
                    warn!("Failed to initialize with partitions.");
                }
            } else {
                warn!("No supported filesystem found in partitions.");
            }
        }
        Ok(_) => {
            warn!("No partitions found.");
        }
        Err(e) => {
            warn!("Failed to scan GPT partitions: {:?}", e);
        }
    }
}
