//! Partition management and filesystem detection
//!
//! This module provides functionality to scan GPT partition tables and detect
//! filesystem types on each partition.

use alloc::{string::String, sync::Arc, vec, vec::Vec};
use axerrno::{AxResult, ax_err};
use axfs_vfs::VfsOps;
use log::{debug, info, warn};

use crate::dev::Disk;

/// Partition information
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// Partition index (0-based)
    pub index: u32,
    /// Partition name
    pub name: String,
    /// Partition type GUID
    pub partition_type_guid: [u8; 16],
    /// Unique partition GUID
    pub unique_partition_guid: [u8; 16],
    /// Starting LBA
    pub starting_lba: u64,
    /// Ending LBA
    pub ending_lba: u64,
    /// Partition size in bytes
    pub size_bytes: u64,
    /// Detected filesystem type
    pub filesystem_type: Option<FilesystemType>,
}

/// Filesystem types that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemType {
    /// FAT32/FAT16 filesystem
    Fat,
    /// ext4/ext3/ext2 filesystem
    Ext4,
    /// Unknown filesystem
    Unknown,
}

/// Simple GPT partition scanner
pub fn scan_gpt_partitions(disk: &mut Disk) -> AxResult<Vec<PartitionInfo>> {
    info!("Scanning for partitions (simplified implementation)...");

    // For now, return a single partition covering the whole disk
    // This is a simplified implementation that doesn't actually parse GPT
    let disk_size = disk.size();

    if disk_size == 0 {
        return Ok(Vec::new());
    }

    // Try to detect filesystem on the whole disk
    let filesystem_type = detect_filesystem_type(disk, 0);

    let partition = PartitionInfo {
        index: 0,
        name: String::from("disk"),
        partition_type_guid: [0; 16],
        unique_partition_guid: [0; 16],
        starting_lba: 0,
        ending_lba: disk_size / 512,
        size_bytes: disk_size,
        filesystem_type,
    };

    info!(
        "Found disk: '{}' ({} bytes) with filesystem: {:?}",
        partition.name, partition.size_bytes, partition.filesystem_type
    );

    Ok(vec![partition])
}

/// Detect filesystem type on a partition
fn detect_filesystem_type(disk: &mut Disk, start_lba: u64) -> Option<FilesystemType> {
    let mut boot_sector = [0u8; 512];

    // Save current position
    let original_position = disk.position();

    // Set position to read from the specific LBA
    disk.set_position(start_lba * 512);

    if let Err(_) = read_exact(disk, &mut boot_sector) {
        warn!("Failed to read boot sector at LBA {}", start_lba);
        // Restore position
        disk.set_position(original_position);
        return None;
    }

    // Restore position
    disk.set_position(original_position);

    // Debug: print first bytes of boot sector
    debug!(
        "Boot sector at LBA {}: first 64 bytes: {:?}",
        start_lba,
        &boot_sector[..64]
    );

    // Check for FAT filesystem
    if is_fat_filesystem(&boot_sector) {
        debug!("Detected FAT filesystem at LBA {}", start_lba);
        return Some(FilesystemType::Fat);
    }

    // Check for ext4 filesystem
    if is_ext4_filesystem(disk, start_lba) {
        debug!("Detected ext4 filesystem at LBA {}", start_lba);
        return Some(FilesystemType::Ext4);
    }

    debug!("Unknown filesystem type at LBA {}", start_lba);
    None
}

/// Read exactly the requested number of bytes
fn read_exact(disk: &mut Disk, mut buf: &mut [u8]) -> Result<(), ()> {
    while !buf.is_empty() {
        match disk.read_one(buf) {
            Ok(0) => break,
            Ok(n) => buf = &mut buf[n..],
            Err(_) => return Err(()),
        }
    }
    Ok(())
}

/// Check if the boot sector indicates a FAT filesystem
fn is_fat_filesystem(boot_sector: &[u8; 512]) -> bool {
    // Check for FAT12/FAT16/FAT32 signature at offset 0x36 (FAT) or 0x52 (FAT32)
    if boot_sector.len() >= 0x36 + 3 {
        let fat_sig = &boot_sector[0x36..0x36 + 3];
        if fat_sig == b"FAT" {
            return true;
        }
    }

    if boot_sector.len() >= 0x52 + 5 {
        let fat32_sig = &boot_sector[0x52..0x52 + 5];
        if fat32_sig == b"FAT32" {
            return true;
        }
    }

    false
}

/// Check if the partition contains an ext4 filesystem
fn is_ext4_filesystem(disk: &mut Disk, start_lba: u64) -> bool {
    // ext4 superblock is at offset 1024 (2 sectors) from the start of the partition
    let superblock_offset = start_lba * 512 + 1024;
    let mut superblock = [0u8; 2048]; // Increase buffer size to accommodate the magic number offset

    // Save current position
    let pos = disk.position();

    // Set position to read the superblock
    disk.set_position(superblock_offset);

    let result = if let Err(_) = read_exact(disk, &mut superblock) {
        warn!(
            "Failed to read ext4 superblock at offset {}",
            superblock_offset
        );
        false
    } else {
        // Check for ext4 magic number (0xEF53) at offset 1080 (0x438) in the superblock
        // But since we're reading from offset 1024, the magic number will be at index 56
        if superblock.len() >= 58 {
            let magic = u16::from_le_bytes([superblock[56], superblock[57]]);
            magic == 0xEF53
        } else {
            false
        }
    };

    // Restore position
    disk.set_position(pos);

    result
}

/// Create a filesystem instance for the given partition and filesystem type
pub fn create_filesystem_for_partition(
    disk: Disk,
    partition: &PartitionInfo,
) -> AxResult<Arc<dyn VfsOps>> {
    match partition.filesystem_type {
        Some(FilesystemType::Fat) => {
            info!("Creating FAT filesystem for partition '{}'", partition.name);
            // Use the whole disk for now

            let fs = crate::fs::fatfs::FatFileSystem::new(disk);
            Ok(Arc::new(fs))
        }
        Some(FilesystemType::Ext4) => {
            info!(
                "Creating ext4 filesystem for partition '{}'",
                partition.name
            );
            // Use the whole disk for now
            let fs = crate::fs::ext4fs::Ext4FileSystem::new(disk);
            Ok(Arc::new(fs))
        }
        Some(FilesystemType::Unknown) | None => {
            warn!("Unknown filesystem type for partition '{}'", partition.name);
            ax_err!(Unsupported, "Unknown filesystem type")
        }
    }
}
