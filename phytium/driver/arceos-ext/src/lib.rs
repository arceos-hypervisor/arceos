#![no_std]

use core::ptr::NonNull;

pub use axstd::io::Error as AxError;

use axstd::os::arceos::modules::{
    axhal::{
        mem::{PhysAddr, phys_to_virt, virt_to_phys},
        paging::MappingFlags,
    },
    axmm::kernel_aspace,
};
use mbarrier::mb;

pub fn iomap(paddr: PhysAddr, size: usize) -> Result<NonNull<u8>, AxError> {
    let vaddr = phys_to_virt(paddr);

    let mut g = kernel_aspace().lock();

    if let Err(e) = g.map_linear(
        vaddr,
        paddr,
        size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
    ) {
        if !matches!(e, AxError::AlreadyExists) {
            return Err(e);
        }
    }

    g.protect(
        vaddr,
        size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
    )?;

    mb();

    Ok(unsafe { NonNull::new_unchecked(vaddr.as_mut_ptr()) })
}

struct Cache;

impl dma_api::Impl for Cache {
    fn map(addr: NonNull<u8>, _size: usize, _direction: dma_api::Direction) -> u64 {
        virt_to_phys((addr.as_ptr() as usize).into()).as_usize() as _
    }

    fn unmap(_addr: NonNull<u8>, _size: usize) {}

    fn flush(addr: NonNull<u8>, size: usize) {
        #[cfg(target_arch = "aarch64")]
        aarch64_cpu_ext::cache::dcache_range(
            aarch64_cpu_ext::cache::CacheOp::Clean,
            addr.as_ptr() as usize,
            size,
        );
    }

    fn invalidate(addr: NonNull<u8>, size: usize) {
        #[cfg(target_arch = "aarch64")]
        aarch64_cpu_ext::cache::dcache_range(
            aarch64_cpu_ext::cache::CacheOp::Invalidate,
            addr.as_ptr() as usize,
            size,
        );
    }
}

dma_api::set_impl!(Cache);
