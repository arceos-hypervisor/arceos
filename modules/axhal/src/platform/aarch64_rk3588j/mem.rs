use crate::mem::{MemRegion, MemRegionFlags, PhysAddr};
use page_table_entry::{GenericPTE, MappingFlags, aarch64::A64PTE};

/// Returns (rk3588j only) memory regions.
pub(crate) fn default_rk3588j_regions() -> impl Iterator<Item = MemRegion> {
    [].into_iter()
}

/// Returns platform-specific memory regions.
pub(crate) fn platform_regions() -> impl Iterator<Item = MemRegion> {
    crate::mem::default_free_regions()
        .chain(default_rk3588j_regions())
        .chain(crate::mem::default_mmio_regions())
}

pub(crate) unsafe fn init_boot_page_table(
    boot_pt_l0: *mut [A64PTE; 512],
    boot_pt_l1: *mut [A64PTE; 512],
) {
    let boot_pt_l0 = &mut *boot_pt_l0;
    let boot_pt_l1 = &mut *boot_pt_l1;
    boot_pt_l0[0] = A64PTE::new_table(PhysAddr::from(boot_pt_l1.as_ptr() as usize));

    boot_pt_l1[0] = A64PTE::new_page(
        PhysAddr::from(0),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
    boot_pt_l1[1] = A64PTE::new_page(
        PhysAddr::from(0x4000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
    boot_pt_l1[2] = A64PTE::new_page(
        PhysAddr::from(0x8000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
    boot_pt_l1[3] = A64PTE::new_page(
        PhysAddr::from(0xC000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        true,
    );
    boot_pt_l1[4] = A64PTE::new_page(
        PhysAddr::from(0x1_0000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
    boot_pt_l1[7] = A64PTE::new_page(
        PhysAddr::from(0x1_C000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
    boot_pt_l1[8] = A64PTE::new_page(
        PhysAddr::from(0x1_F000_0000),
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true,
    );
}
