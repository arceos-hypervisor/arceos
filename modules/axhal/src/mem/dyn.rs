extern crate alloc;

use core::ptr::NonNull;

use axplat_dyn::mem::{
    percpu_data,
    region::{AccessFlags, MemRegionKind},
};
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr};
use page_table_entry::MappingFlags;

use super::{AddrMapFunc, MemRegion, MemRegionFlags};

static mut MAP_FUNC: AddrMapFunc = |_start_vaddr, _start_paddr, _size, _flags| Ok(());

/// Converts a virtual address to a physical address.
#[inline]
pub fn virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    axplat_dyn::mem::virt_to_phys(vaddr.as_usize().into())
        .raw()
        .into()
}

/// Converts a physical address to a virtual address.
#[inline]
pub fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    axplat_dyn::mem::phys_to_virt(paddr.as_usize().into())
        .raw()
        .into()
}

/// Returns an iterator over all physical memory regions.
pub fn memory_regions() -> impl Iterator<Item = MemRegion> {
    axplat_dyn::mem::memory_regions().map(|reg| reg.into())
}

impl From<&axplat_dyn::mem::MemRegion> for MemRegion {
    fn from(value: &axplat_dyn::mem::MemRegion) -> Self {
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

pub(crate) unsafe fn init_map_liner(f: AddrMapFunc) {
    unsafe {
        MAP_FUNC = f;
    }
}
/// maps a mmio physical address to a virtual address.
pub fn iomap(addr: PhysAddr, size: usize) -> Result<NonNull<u8>, axerrno::AxError> {
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
        )?;
    }
    Ok(NonNull::new(start_virt.as_mut_ptr()).unwrap())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _percpu_base() -> *mut u8 {
    unsafe { percpu_data().as_ref().as_ptr() as usize as _ }
}
