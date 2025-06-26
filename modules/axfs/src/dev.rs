use axdriver::prelude::*;

const BLOCK_SIZE: usize = 512;

/// A disk device with a cursor and block cache.
pub struct Disk {
    block_id: u64,
    offset: usize,
    dev: AxBlockDevice,
    // Block cache to avoid repeated reads of the same block
    cached_block_id: Option<u64>,
    cached_block_data: [u8; BLOCK_SIZE],
}

impl Disk {
    /// Create a new disk.
    pub fn new(dev: AxBlockDevice) -> Self {
        assert_eq!(BLOCK_SIZE, dev.block_size());
        Self {
            block_id: 0,
            offset: 0,
            dev,
            cached_block_id: None,
            cached_block_data: [0; BLOCK_SIZE],
        }
    }

    /// Get the size of the disk.
    pub fn size(&self) -> u64 {
        self.dev.num_blocks() * BLOCK_SIZE as u64
    }

    /// Get the position of the cursor.
    pub fn position(&self) -> u64 {
        self.block_id * BLOCK_SIZE as u64 + self.offset as u64
    }

    /// Set the position of the cursor.
    pub fn set_position(&mut self, pos: u64) {
        let new_block_id = pos / BLOCK_SIZE as u64;
        let new_offset = pos as usize % BLOCK_SIZE;

        // If we're moving to a different block, we might want to keep the cache
        // Only invalidate if we're jumping far away (more than 1 block difference)
        if let Some(cached_id) = self.cached_block_id {
            if new_block_id.abs_diff(cached_id) > 1 {
                self.cached_block_id = None;
            }
        }

        self.block_id = new_block_id;
        self.offset = new_offset;
    }

    /// Read within one block, returns the number of bytes read.
    pub fn read_one(&mut self, buf: &mut [u8]) -> DevResult<usize> {
        let read_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            // whole block - read directly without caching
            self.dev
                .read_block(self.block_id, &mut buf[0..BLOCK_SIZE])?;
            self.block_id += 1;
            BLOCK_SIZE
        } else {
            // partial block - use cache to avoid repeated reads
            let start = self.offset;
            let count = buf.len().min(BLOCK_SIZE - self.offset);

            // Check if we need to read the block into cache
            if self.cached_block_id != Some(self.block_id) {
                // Cache miss - read the block
                self.dev.read_block(self.block_id, &mut self.cached_block_data)?;
                self.cached_block_id = Some(self.block_id);
            }

            // Copy data from cache
            buf[..count].copy_from_slice(&self.cached_block_data[start..start + count]);

            self.offset += count;
            if self.offset >= BLOCK_SIZE {
                self.block_id += 1;
                self.offset -= BLOCK_SIZE;
            }
            count
        };
        trace!(
            "Disk::read_one: block_id={}, offset={}, read_size={}",
            self.block_id,
            self.offset,
            read_size
        );
        Ok(read_size)
    }

    /// Write within one block, returns the number of bytes written.
    pub fn write_one(&mut self, buf: &[u8]) -> DevResult<usize> {
        let write_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            // whole block - write directly and invalidate cache
            self.dev.write_block(self.block_id, &buf[0..BLOCK_SIZE])?;
            // Invalidate cache for this block
            if self.cached_block_id == Some(self.block_id) {
                self.cached_block_id = None;
            }
            self.block_id += 1;
            BLOCK_SIZE
        } else {
            // partial block - use cache for read-modify-write
            let start = self.offset;
            let count = buf.len().min(BLOCK_SIZE - self.offset);

            // Check if we need to read the block into cache
            if self.cached_block_id != Some(self.block_id) {
                // Cache miss - read the block
                self.dev.read_block(self.block_id, &mut self.cached_block_data)?;
                self.cached_block_id = Some(self.block_id);
            }

            // Modify data in cache
            self.cached_block_data[start..start + count].copy_from_slice(&buf[..count]);

            // Write the modified block back
            self.dev.write_block(self.block_id, &self.cached_block_data)?;

            self.offset += count;
            if self.offset >= BLOCK_SIZE {
                self.block_id += 1;
                self.offset -= BLOCK_SIZE;
            }
            count
        };
        trace!(
            "Disk::write_one: block_id={}, offset={}, write_size={}",
            self.block_id,
            self.offset,
            write_size
        );
        Ok(write_size)
    }
}
