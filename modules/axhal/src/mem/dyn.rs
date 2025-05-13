use core::{error::Error, ptr::NonNull};

use alloc::{boxed::Box, format};
use axerrno::AxError;
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr};
use page_table_entry::MappingFlags;
use somehal::mem::region::{AccessFlags, MemRegionKind};

use super::{MemRegion, MemRegionFlags};

static mut MAP_FUNC: MapLinearFunc = |_start_vaddr, _start_paddr, _size, _flags| Ok(());

pub type MapLinearFunc = fn(
    start_vaddr: VirtAddr,
    start_paddr: PhysAddr,
    size: usize,
    flags: MappingFlags,
) -> Result<(), AxError>;

/// Converts a virtual address to a physical address.
#[inline]
pub fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    somehal::mem::virt_to_phys(vaddr.as_usize().into())
        .raw()
        .into()
}

/// Converts a physical address to a virtual address.
#[inline]
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    somehal::mem::phys_to_virt(paddr.as_usize().into())
        .raw()
        .into()
}

/// Returns an iterator over all physical memory regions.
pub fn memory_regions() -> impl Iterator<Item = MemRegion> {
    somehal::mem::memory_regions().map(|reg| reg.into())
}

impl From<&somehal::mem::MemRegion> for MemRegion {
    fn from(value: &somehal::mem::MemRegion) -> Self {
        let mut flags = MemRegionFlags::empty();
        if value.config.access.contains(AccessFlags::Read) {
            flags |= MemRegionFlags::READ;
        }
        if value.config.access.contains(AccessFlags::Write) {
            flags |= MemRegionFlags::WRITE;
        }
        if value.config.access.contains(AccessFlags::Execute) {
            flags |= MemRegionFlags::EXECUTE;
        }
        match value.kind {
            MemRegionKind::Code => {
                flags |= MemRegionFlags::RESERVED;
            }
            MemRegionKind::Stack => {
                flags |= MemRegionFlags::RESERVED;
            }
            MemRegionKind::PerCpu => {
                flags |= MemRegionFlags::RESERVED;
            }
            MemRegionKind::Device => {
                flags |= MemRegionFlags::DEVICE;
            }
            MemRegionKind::Memory => {
                flags |= MemRegionFlags::FREE;
            }
            MemRegionKind::Reserved => {
                flags |= MemRegionFlags::RESERVED;
            }
        }

        Self {
            paddr: value.phys_start.raw().into(),
            size: value.size,
            flags,
            name: value.name,
        }
    }
}

pub(crate) unsafe fn init_map_liner(f: MapLinearFunc) {
    unsafe {
        MAP_FUNC = f;
    }
}

pub fn iomap(addr: PhysAddr, size: usize) -> Result<NonNull<u8>, Box<dyn Error>> {
    let end = (addr.as_usize() + size).align_up_4k();
    let start = addr.align_down_4k();
    let size = end - start.as_usize();

    let start_virt = phys_to_virt(start);

    unsafe {
        MAP_FUNC(
            start_virt,
            addr,
            size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        )
        .map_err(|e| format!("Failed to map memory: {}", e))?;
    }
    Ok(NonNull::new(start_virt.as_mut_ptr()).unwrap())
}
