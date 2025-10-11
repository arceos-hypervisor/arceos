use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};
use axdriver_block::BlockDriverOps;
use gpt_disk_io::{BlockIo, Disk};
use rdif_block::BlkError;
use rdrive::{Device, DeviceGuard};
use spin::Mutex;

#[cfg(feature = "virtio-blk")]
mod virtio;

pub struct Block {
    dev: Device<rdif_block::Block>,
    queue: Mutex<rdif_block::CmdQueue>,
}

impl Block {
    pub fn is_gpt_partition(&mut self) -> bool {
        let mut disk = match Disk::new(BlockDriverAdapter(self)) {
            Ok(d) => d,
            Err(e) => {
                return false;
            }
        };

        let mut block_buf = [0u8; 512];

        let primary_header = match disk.read_primary_gpt_header(&mut block_buf) {
            Ok(h) => h,
            Err(_) => return false,
        };

        primary_header.is_signature_valid()
    }
}

struct BlockDriverAdapter<'a>(&'a mut Block);

impl<'a> BlockIo for BlockDriverAdapter<'a> {
    type Error = DevError;

    fn block_size(&self) -> gpt_disk_io::gpt_disk_types::BlockSize {
        gpt_disk_io::gpt_disk_types::BlockSize::from_usize(self.0.block_size()).unwrap()
    }

    fn num_blocks(&mut self) -> Result<u64, Self::Error> {
        Ok(self.0.num_blocks())
    }

    fn read_blocks(
        &mut self,
        start_lba: gpt_disk_io::gpt_disk_types::Lba,
        dst: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.0.read_block(start_lba.to_u64(), dst)
    }

    fn write_blocks(
        &mut self,
        start_lba: gpt_disk_io::gpt_disk_types::Lba,
        src: &[u8],
    ) -> Result<(), Self::Error> {
        self.0.write_block(start_lba.to_u64(), src)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush()
    }
}

impl BaseDriverOps for Block {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn device_name(&self) -> &str {
        self.dev.descriptor().name
    }
}

impl BlockDriverOps for Block {
    fn num_blocks(&self) -> u64 {
        self.queue.lock().num_blocks() as _
    }

    fn block_size(&self) -> usize {
        self.queue.lock().block_size()
    }

    fn flush(&mut self) -> DevResult {
        Ok(())
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        let blk_count = buf.len() / self.block_size();
        let blocks = self
            .queue
            .lock()
            .read_blocks_blocking(block_id as _, blk_count);
        for (block, chunk) in blocks.into_iter().zip(buf.chunks_mut(self.block_size())) {
            let block = block.map_err(maping_blk_err_to_dev_err)?;
            if block.len() != chunk.len() {
                return Err(DevError::Io);
            }
            chunk.copy_from_slice(&block);
        }
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        let blocks = self.queue.lock().write_blocks_blocking(block_id as _, buf);
        for block in blocks {
            block.map_err(maping_blk_err_to_dev_err)?;
        }
        Ok(())
    }
}

impl From<Device<rdif_block::Block>> for Block {
    fn from(base: Device<rdif_block::Block>) -> Self {
        let queue = base.lock().unwrap().create_queue().unwrap();
        Self {
            dev: base,
            queue: Mutex::new(queue),
        }
    }
}

fn maping_blk_err_to_dev_err(err: BlkError) -> DevError {
    match err {
        BlkError::NotSupported => DevError::Unsupported,
        BlkError::Retry => DevError::Again,
        BlkError::NoMemory => DevError::NoMemory,
        BlkError::InvalidBlockIndex(_) => DevError::InvalidParam,
        BlkError::Other(error) => {
            error!("Block device error: {error}");
            DevError::Io
        }
    }
}

fn maping_dev_err_to_blk_err(err: DevError) -> BlkError {
    match err {
        DevError::Again => BlkError::Retry,
        DevError::AlreadyExists => BlkError::Other("Already exists".into()),
        DevError::BadState => BlkError::Other("Bad internal state".into()),
        DevError::InvalidParam => BlkError::Other("Invalid parameter".into()),
        DevError::Io => BlkError::Other("I/O error".into()),
        DevError::NoMemory => BlkError::NoMemory,
        DevError::ResourceBusy => BlkError::Other("Resource busy".into()),
        DevError::Unsupported => BlkError::NotSupported,
    }
}

pub trait PlatformDeviceBlock {
    fn register_block<T: rdif_block::Interface>(self, dev: T);
}

impl PlatformDeviceBlock for rdrive::PlatformDevice {
    fn register_block<T: rdif_block::Interface>(self, dev: T) {
        let dev = rdif_block::Block::new(dev);
        self.register(dev);
    }
}
