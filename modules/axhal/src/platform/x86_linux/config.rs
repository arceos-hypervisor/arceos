use core::fmt::{Debug, Formatter, Result};
use core::{mem::size_of, slice};

const CONFIG_SIGNATURE: [u8; 6] = *b"ARCEOS";
const CONFIG_REVISION: u16 = 13;

const HV_CELL_NAME_MAXLEN: usize = 31;

/// The jailhouse cell configuration.
///
/// @note Keep Config._HEADER_FORMAT in jailhouse-cell-linux in sync with this
/// structure.
#[derive(Debug)]
#[repr(C, packed)]
pub struct HvCellDesc {
    signature: [u8; 6],
    revision: u16,
    name: [u8; HV_CELL_NAME_MAXLEN + 1],
    id: u32, // set by the driver
    num_memory_regions: u32,
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct HvMemoryRegion {
    pub phys_start: u64,
    pub virt_start: u64,
    pub size: u64,
    pub flags: u64,
}

/// General descriptor of the system.
#[derive(Debug)]
#[repr(C)]
pub struct HvSystemConfig {
    pub signature: [u8; 6],
    pub revision: u16,
    /// AxVisor location in memory
    pub hypervisor_memory: HvMemoryRegion,
    /// RTOS location in memory
    pub rtos_memory: HvMemoryRegion,
    pub root_cell: HvCellDesc,
    // CellConfigLayout placed here.
}

// /// A dummy layout with all variant-size fields empty.
// #[derive(Debug)]
// #[repr(C, packed)]
// struct CellConfigLayout {
//     mem_regions: [HvMemoryRegion; 0],
// }

pub struct CellConfig<'a> {
    desc: &'a HvCellDesc,
}

impl HvCellDesc {
    pub const fn config(&self) -> CellConfig {
        CellConfig::from(self)
    }

    pub const fn config_size(&self) -> usize {
        self.num_memory_regions as usize * size_of::<HvMemoryRegion>()
    }
}

impl HvSystemConfig {
    pub fn get<'a>() -> &'a Self {
        unsafe { &*super::consts::hv_config_ptr() }
    }

    pub const fn size(&self) -> usize {
        size_of::<Self>() + self.root_cell.config_size()
    }

    pub fn check(&self) {
        assert_eq!(self.signature, CONFIG_SIGNATURE);
        assert_eq!(self.revision, CONFIG_REVISION);
    }
}

impl<'a> CellConfig<'a> {
    const fn from(desc: &'a HvCellDesc) -> Self {
        Self { desc }
    }

    fn config_ptr<T>(&self) -> *const T {
        unsafe { (self.desc as *const HvCellDesc).add(1) as _ }
    }

    pub const fn size(&self) -> usize {
        self.desc.config_size()
    }

    pub fn mem_regions(&self) -> &'static [HvMemoryRegion] {
        // XXX: data may unaligned, which cause panic on debug mode. Same below.
        // See: https://doc.rust-lang.org/src/core/slice/mod.rs.html#6435-6443
        unsafe {
            let ptr = self.config_ptr() as _;
            slice::from_raw_parts(ptr, self.desc.num_memory_regions as usize)
        }
    }
}

impl Debug for CellConfig<'_> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let name = self.desc.name;
        let mut len = 0;
        while name[len] != 0 {
            len += 1;
        }
        f.debug_struct("CellConfig")
            .field("name", &core::str::from_utf8(&name[..len]))
            .field("size", &self.size())
            .field("mem regions", &self.mem_regions())
            .finish()
    }
}
