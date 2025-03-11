use memory_addr::MemoryAddr;

use super::config::HvSystemConfig;
use super::header::HvHeader;
use super::percpu::PerCpu;
use crate::mem::VirtAddr;

/// Size of the hypervisor heap.
pub const HV_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32 MB

/// Size of the per-CPU data (stack and other CPU-local data).
pub const PER_CPU_SIZE: usize = 512 * 1024; // 512 KB

/// Start virtual address of the hypervisor memory.
pub const HV_BASE: usize = 0xffff_ff00_0000_0000;

/// Pointer of the `HvHeader` structure.
pub const HV_HEADER_PTR: *const HvHeader = __header_start as _;

/// Pointer of the per-CPU data array.
pub const PER_CPU_ARRAY_PTR: *mut PerCpu = _ekernel as _;

/// Pointer of the `HvSystemConfig` structure.
pub fn hv_config_ptr() -> *const HvSystemConfig {
    (PER_CPU_ARRAY_PTR as usize + HvHeader::get().max_cpus as usize * PER_CPU_SIZE) as _
}

/// Pointer of the free memory pool.
pub fn free_memory_start() -> VirtAddr {
    VirtAddr::from(hv_config_ptr() as usize + HvSystemConfig::get().size()).align_up_4k()
    // align_up(hv_config_ptr() as usize + HvSystemConfig::get().size())
}

/// End virtual address of the hypervisor memory.
pub fn hv_end() -> VirtAddr {
    VirtAddr::from(HV_BASE + HvSystemConfig::get().hypervisor_memory.size as usize)
}

unsafe extern "C" {
    unsafe fn __header_start();
    unsafe fn _ekernel();
}
