use memory_addr::MemoryAddr;

use super::config::HvSystemConfig;
use super::header::HvHeader;
use crate::mem::VirtAddr;
use axconfig::plat::KERNEL_BASE_VADDR;

/// Pointer of the `HvHeader` structure.
pub const HV_HEADER_PTR: *const HvHeader = __header_start as _;

/// Pointer of the `HvSystemConfig` structure.
pub fn hv_config_ptr() -> *const HvSystemConfig {
    _ekernel as _
}

/// Pointer of the free memory pool.
pub fn free_memory_start() -> VirtAddr {
    VirtAddr::from(hv_config_ptr() as usize + HvSystemConfig::get().size()).align_up_4k()
}

/// End virtual address of the hypervisor memory.
pub fn hv_end() -> VirtAddr {
    VirtAddr::from(KERNEL_BASE_VADDR + HvSystemConfig::get().hypervisor_memory.size as usize)
}

unsafe extern "C" {
    unsafe fn __header_start();
    unsafe fn _ekernel();
}
