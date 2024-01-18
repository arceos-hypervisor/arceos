use alloc::collections::BTreeMap;
use core::{fmt::{Debug, Formatter, Result}, marker::PhantomData};

use libax::hv::{HyperCraftHal, HyperCraftHalImpl, GuestPhysAddr, HostPhysAddr, HostVirtAddr, phys_to_virt, virt_to_phys, Result as HyperResult, Error, GuestPageTable, GuestPageTableTrait, PerCpuDevices, VCpu, VmExitInfo, PerVmDevices};

use page_table_entry::MappingFlags;
use vm_config::VmConfigEntry;
use alloc::vec::Vec;

macro_rules! cfg_block {
    ($( #[$meta:meta] {$($item:item)*} )*) => {
        $($(
            #[$meta]
            $item
        )*)*
    }
}

// about guests
pub const BIOS_PADDR: HostPhysAddr = 0x400_0000;
pub const BIOS_SIZE: usize = 0x2000;
cfg_block! {
    #[cfg(feature = "guest_nimbos")]
    {
        pub const BIOS_ENTRY: GuestPhysAddr = 0x8000;
        pub const GUEST_ENTRY: GuestPhysAddr = 0x20_0000;

        pub const GUEST_IMAGE_PADDR: HostPhysAddr = 0x400_1000;
        pub const GUEST_IMAGE_SIZE: usize = 0x10_0000; // 1M
    }
    #[cfg(feature = "guest_linux")]
    {
        pub const BIOS_ENTRY: GuestPhysAddr = 0x7c00;
    }
}
pub const GUEST_PHYS_MEMORY_BASE: GuestPhysAddr = 0;
pub const GUEST_PHYS_MEMORY_SIZE: usize = 0x100_0000; // 16M

pub const fn is_aligned(addr: usize) -> bool {
    (addr & (HyperCraftHalImpl::PAGE_SIZE - 1)) == 0
}

#[derive(Debug)]
enum Mapper {
    Offset(usize),
}

#[derive(Debug)]
pub struct GuestMemoryRegion {
    pub gpa: GuestPhysAddr,
    pub hpa: HostPhysAddr,
    pub size: usize,
    pub flags: MappingFlags,
}

pub struct MapRegion {
    pub start: GuestPhysAddr,
    pub size: usize,
    pub flags: MappingFlags,
    mapper: Mapper,
}

impl MapRegion {
    pub fn new_offset(
        start_gpa: GuestPhysAddr,
        start_hpa: HostPhysAddr,
        size: usize,
        flags: MappingFlags,
    ) -> Self {
        assert!(is_aligned(start_gpa));
        assert!(is_aligned(start_hpa));
        assert!(is_aligned(size));
        let offset = start_gpa - start_hpa;
        Self {
            start: start_gpa,
            size,
            flags,
            mapper: Mapper::Offset(offset),
        }
    }

    fn is_overlap_with(&self, other: &Self) -> bool {
        let s0 = self.start;
        let e0 = s0 + self.size;
        let s1 = other.start;
        let e1 = s1 + other.size;
        !(e0 <= s1 || e1 <= s0)
    }

    fn target(&self, gpa: GuestPhysAddr) -> HostPhysAddr {
        match self.mapper {
            Mapper::Offset(off) => gpa.wrapping_sub(off),
        }
    }

    fn map_to(&self, npt: &mut GuestPageTable) -> HyperResult {
        let mut start = self.start;
        let end = start + self.size;
        while start < end {
            let target = self.target(start);
            npt.map(start, target, self.flags)?;
            start += HyperCraftHalImpl::PAGE_SIZE;
        }
        Ok(())
    }

    fn unmap_to(&self, npt: &mut GuestPageTable) -> HyperResult {
        let mut start = self.start;
        let end = start + self.size;
        while start < end {
            npt.unmap(start)?;
            start += HyperCraftHalImpl::PAGE_SIZE;
        }
        Ok(())
    }
}

impl Debug for MapRegion {
    fn fmt(&self, f: &mut Formatter) -> Result {
        f.debug_struct("MapRegion")
            .field("range", &(self.start..self.start + self.size))
            .field("size", &self.size)
            .field("flags", &self.flags)
            .field("mapper", &self.mapper)
            .finish()
    }
}

impl From<GuestMemoryRegion> for MapRegion {
    fn from(r: GuestMemoryRegion) -> Self {
        Self::new_offset(r.gpa, r.hpa, r.size, r.flags)
    }
}

pub struct GuestPhysMemorySet {
    regions: BTreeMap<GuestPhysAddr, MapRegion>,
    npt: GuestPageTable,
}

impl GuestPhysMemorySet {
    pub fn new() -> HyperResult<Self> {
        Ok(Self {
            npt: GuestPageTable::new()?,
            regions: BTreeMap::new(),
        })
    }

    pub fn nest_page_table_root(&self) -> HostPhysAddr {
        self.npt.root_paddr().into()
    }

    fn test_free_area(&self, other: &MapRegion) -> bool {
        if let Some((_, before)) = self.regions.range(..other.start).last() {
            if before.is_overlap_with(other) {
                return false;
            }
        }
        if let Some((_, after)) = self.regions.range(other.start..).next() {
            if after.is_overlap_with(other) {
                return false;
            }
        }
        true
    }

    pub fn map_region(&mut self, region: MapRegion) -> HyperResult {
        if region.size == 0 {
            return Ok(());
        }
        if !self.test_free_area(&region) {
            warn!(
                "MapRegion({:#x}..{:#x}) overlapped in:\n{:#x?}",
                region.start,
                region.start + region.size,
                self
            );
            return Err(Error::InvalidParam);
        }
        region.map_to(&mut self.npt)?;
        self.regions.insert(region.start, region);
        Ok(())
    }

    pub fn clear(&mut self) {
        for region in self.regions.values() {
            region.unmap_to(&mut self.npt).unwrap();
        }
        self.regions.clear();
    }
}

impl Drop for GuestPhysMemorySet {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Debug for GuestPhysMemorySet {
    fn fmt(&self, f: &mut Formatter) -> Result {
        f.debug_struct("GuestPhysMemorySet")
            .field("page_table_root", &self.nest_page_table_root())
            .field("regions", &self.regions)
            .finish()
    }
}

#[repr(align(4096))]
pub(super) struct AlignedMemory<const LEN: usize>([u8; LEN]);

pub(super) static mut GUEST_PHYS_MEMORY: [AlignedMemory<GUEST_PHYS_MEMORY_SIZE>; 2] =
    [AlignedMemory([0; GUEST_PHYS_MEMORY_SIZE]), AlignedMemory([0; GUEST_PHYS_MEMORY_SIZE])];

fn gpa_as_mut_ptr(id: usize, guest_paddr: GuestPhysAddr) -> *mut u8 {
    let offset = unsafe { &(GUEST_PHYS_MEMORY[id]) as *const _ as usize };
    let host_vaddr = guest_paddr + offset;
    host_vaddr as *mut u8
}

#[cfg(target_arch = "x86_64")]
fn load_guest_image(id: usize, hpa: HostPhysAddr, load_gpa: GuestPhysAddr, size: usize) {
    let image_ptr = usize::from(phys_to_virt(hpa.into())) as *const u8;
    let image = unsafe { core::slice::from_raw_parts(image_ptr, size) };

    trace!("loading to guest memory: host {:#x} to guest {:#x}, size {:#x}", image_ptr as usize, load_gpa, size);

    unsafe {
        core::slice::from_raw_parts_mut(gpa_as_mut_ptr(id, load_gpa), size).copy_from_slice(image)
    }
}

#[cfg(target_arch = "x86_64")]
pub fn setup_gpm(id: usize, config: VmConfigEntry) -> HyperResult<GuestPhysMemorySet> {
    let image_config = &config.image;
    // copy BIOS and guest images
    // load_guest_image(id, BIOS_PADDR, BIOS_ENTRY, BIOS_SIZE);
    load_guest_image(id, image_config.bios_paddr, image_config.bios_entry, image_config.bios_size);
    #[cfg(feature = "guest_nimbos")]
    {
        load_guest_image(id, GUEST_IMAGE_PADDR, GUEST_ENTRY, GUEST_IMAGE_SIZE);
    }
    
    // create nested page table and add mapping
    let mut gpm = GuestPhysMemorySet::new()?;
    // the first region is physical memory, skip it and manually map it by virt_to_phys
    let memory_config = &config.memory;
    let guest_memory_regions: Vec<GuestMemoryRegion> = memory_config.regions.iter().skip(1).map(|region| {
        GuestMemoryRegion {
            gpa: region.ipa_start,
            hpa: region.pa_start,
            size: region.length,
            flags: MappingFlags::from_bits(region.flags).unwrap_or_else(|| panic!("Invalid flags")),
        }
    }).collect();
    for r in guest_memory_regions {
        gpm.map_region(r.into())?;
    }
    let physical_memory_region = memory_config.regions.first().unwrap();
    let guest_physical_memory = GuestMemoryRegion {
        gpa: physical_memory_region.ipa_start,
        hpa: virt_to_phys((gpa_as_mut_ptr(id, physical_memory_region.ipa_start) as HostVirtAddr).into()).into(),
        size: physical_memory_region.length,
        flags: MappingFlags::from_bits(physical_memory_region.flags).unwrap_or_else(|| panic!("Invalid flags")),
    };
    gpm.map_region(guest_physical_memory.into())?;

    /* 
    let guest_memory_regions = [
        GuestMemoryRegion {
            // Low RAM
            gpa: GUEST_PHYS_MEMORY_BASE,
            hpa: virt_to_phys((gpa_as_mut_ptr(id, GUEST_PHYS_MEMORY_BASE) as HostVirtAddr).into()).into(),
            size: GUEST_PHYS_MEMORY_SIZE,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        },
        #[cfg(feature = "guest_linux")]
        GuestMemoryRegion {
            // Low RAM2
            gpa: 0x100_0000,
            hpa: 0x6100_0000,
            size: 0xf00_0000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        },
        #[cfg(feature = "guest_linux")]
        GuestMemoryRegion {
            // RAM
            gpa: 0x7000_0000,
            hpa: 0x7000_0000,
            size: 0x1000_0000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        },
        GuestMemoryRegion {
            // PCI
            gpa: 0x8000_0000,
            hpa: 0x8000_0000,
            size: 0x1000_0000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
        GuestMemoryRegion {
            gpa: 0xfe00_0000,
            hpa: 0xfe00_0000,
            size: 0x1_0000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
        GuestMemoryRegion {
            gpa: 0xfeb0_0000,
            hpa: 0xfeb0_0000,
            size: 0x10_0000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
        GuestMemoryRegion {
            // IO APIC
            gpa: 0xfec0_0000,
            hpa: 0xfec0_0000,
            size: 0x1000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
        GuestMemoryRegion {
            // HPET
            gpa: 0xfed0_0000,
            hpa: 0xfed0_0000,
            size: 0x1000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
        GuestMemoryRegion {
            // Local APIC
            gpa: 0xfee0_0000,
            hpa: 0xfee0_0000,
            size: 0x1000,
            flags: MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
        },
    ];
    for r in guest_memory_regions.into_iter() {
        gpm.map_region(r.into())?;
    }
    */
    Ok(gpm)
}
