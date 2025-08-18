// TODO: get memory regions from multiboot info.

use crate::mem::{MemRegion, MemRegionFlags, MemoryAddr, PhysAddr, virt_to_phys};

use super::config::HvSystemConfig;

// MemRegion {
//     paddr: virt_to_phys((_stext as usize).into()),
//     size: _etext as usize - _stext as usize,
//     flags: MemRegionFlags::RESERVED | MemRegionFlags::READ | MemRegionFlags::EXECUTE,
//     name: ".text",
// },

unsafe extern "C" {
    unsafe fn __header_start();
    unsafe fn __header_end();
}

/// Returns the free memory regions reserved by kernel cmdline
/// (not operated by host Linux).
/// The mapping of this memory region should be set up by ArceOS itself,
/// so this region can not be used until `axmm::init_memory_management();` is called.
///
/// This region is added to glocal allocator by `init_allocator_late()`.
fn platform_free_regions() -> impl Iterator<Item = MemRegion> {
    axconfig::devices::MEMORY_REGIONS
        .iter()
        .map(|reg| MemRegion {
            paddr: reg.0.into(),
            size: reg.1,
            flags: MemRegionFlags::FREE
                | MemRegionFlags::RESERVED // Mark as reserved to avoid being used by `init_allocator()`
                | MemRegionFlags::READ
                | MemRegionFlags::WRITE,
            name: "platform free memory",
        })
}

/// Returns the vmm free memory regions (kernel image end to physical memory end).
/// This memory region is used for physical memory allocation before Paging is enabled.
/// (The mapping is set up by host Linux)
fn vmm_free_regions() -> impl Iterator<Item = MemRegion> {
    let mem_pool_start = super::consts::free_memory_start();
    let mem_pool_end = super::consts::hv_end().align_down_4k();
    let mem_pool_size = mem_pool_end.as_usize() - mem_pool_start.as_usize();
    core::iter::once(MemRegion {
        paddr: virt_to_phys(mem_pool_start),
        size: mem_pool_size,
        flags: MemRegionFlags::FREE | MemRegionFlags::READ | MemRegionFlags::WRITE,
        name: "free memory",
    })
    .chain(platform_free_regions())
}

fn vmm_cfg_regions() -> impl Iterator<Item = MemRegion> {
    let vmm_cfg_start = super::consts::cfg_region_start();
    let vmm_cfg_end = super::consts::free_memory_start();
    let vmm_cfg_size = vmm_cfg_end.as_usize() - vmm_cfg_start.as_usize();

    core::iter::once(MemRegion {
        paddr: virt_to_phys(vmm_cfg_start),
        size: vmm_cfg_size,
        // Provided by host, read-only.
        flags: MemRegionFlags::RESERVED | MemRegionFlags::READ,
        name: "System config (for VMM)",
    })
}

pub fn host_memory_regions() -> impl Iterator<Item = MemRegion> {
    let sys_config = HvSystemConfig::get();
    let cell_config = &sys_config.root_cell.config();

    cell_config.mem_regions().iter().map(|region| MemRegion {
        paddr: PhysAddr::from(region.phys_start as usize),
        size: region.size as usize,
        flags: region.flags.clone().into(),
        name: "Linux mem",
    })
}

/// Returns platform-specific memory regions.
pub(crate) fn platform_regions() -> impl Iterator<Item = MemRegion> {
    // Add region for HvHeader.
    // See modules/axhal/linker_hv.lds.S for details.
    core::iter::once(MemRegion {
        paddr: virt_to_phys((__header_start as usize).into()),
        size: __header_end as usize - __header_start as usize,
        flags: MemRegionFlags::RESERVED | MemRegionFlags::READ,
        name: ".header (for VMM)",
    })
    .chain(vmm_cfg_regions())
    .chain(vmm_free_regions())
    // Here we do not use the `default_free_regions`` from  `crate::mem`.
    // Cause we need to reserved regions for
    // per-CPU data and `HvSystemConfig`
    .chain(crate::mem::default_mmio_regions())
    .chain(host_memory_regions().filter(|region| {
        // Map all guest RAM to directly access in hypervisor.
        region.flags.contains(MemRegionFlags::DEVICE)
    }))
}
