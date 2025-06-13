use alloc::boxed::Box;

use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};
use axdriver_block::BlockDriverOps;
use rdrive::{DeviceWeak, block::*};

#[cfg(all(target_arch = "aarch64", feature = "rk3568-emmc"))]
mod rockchip_dwcmshc_sdhci;

#[cfg(all(target_arch = "aarch64", feature = "phytium-sdcard"))]
mod phytium_sdcard;

pub struct Block(DeviceWeak<Box<dyn Interface + 'static>>);

impl BaseDriverOps for Block {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
    fn device_name(&self) -> &str {
        self.0.descriptor.name
    }
}

impl BlockDriverOps for Block {
    fn num_blocks(&self) -> u64 {
        self.0.spin_try_borrow_by(0.into()).unwrap().num_blocks() as _
    }
    fn block_size(&self) -> usize {
        self.0.spin_try_borrow_by(0.into()).unwrap().block_size()
    }
    fn flush(&mut self) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .flush()
            .map_err(convert_error)
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .read_block(block_id as _, buf)
            .map_err(convert_error)
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        self.0
            .spin_try_borrow_by(0.into())
            .unwrap()
            .write_block(block_id as _, buf)
            .map_err(convert_error)
    }
}

impl From<DeviceWeak<Box<dyn Interface + 'static>>> for Block {
    fn from(base: DeviceWeak<Box<dyn Interface + 'static>>) -> Self {
        Self(base)
    }
}

fn convert_error(err: io::Error) -> DevError {
    match err.kind {
        io::ErrorKind::Other(_error) => DevError::Io,
        io::ErrorKind::NotAvailable => DevError::BadState,
        io::ErrorKind::BrokenPipe => DevError::BadState,
        io::ErrorKind::InvalidParameter { name: _ } => DevError::InvalidParam,
        io::ErrorKind::InvalidData => DevError::InvalidParam,
        io::ErrorKind::TimedOut => DevError::Io,
        io::ErrorKind::Interrupted => DevError::Again,
        io::ErrorKind::Unsupported => DevError::Unsupported,
        io::ErrorKind::OutOfMemory => DevError::NoMemory,
        io::ErrorKind::WriteZero => DevError::InvalidParam,
    }
}
