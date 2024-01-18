use heapless::Vec;
use crate::from_hex;

const MAX_VM_MEMORY_REGION: usize = 16;

/// The whole memory region for vm
#[derive(Clone, Copy, Debug, Eq, serde::Deserialize)]
pub struct VmMemoryRegion {
    /// gpa/hva start address
    #[serde(deserialize_with = "from_hex")]
    pub ipa_start: usize,
    /// hpa start address
    #[serde(deserialize_with = "from_hex")]
    pub pa_start: usize,
    /// length of the memory region
    #[serde(deserialize_with = "from_hex")]
    pub length: usize,
    /// flags of the memory region
    #[serde(deserialize_with = "from_hex")]
    pub flags: usize,
}


impl VmMemoryRegion {
    pub const fn default() -> VmMemoryRegion {
        VmMemoryRegion {
            ipa_start: 0,
            pa_start: 0,
            length: 0,
            flags: 0,
        }
    }
    pub fn new(ipa_start:usize, pa_start:usize, length: usize, flags:usize) -> VmMemoryRegion {
        VmMemoryRegion {
            ipa_start: ipa_start,
            pa_start: pa_start,
            length: length,
            flags: flags,
        }
    }
}

impl PartialEq for VmMemoryRegion {
    fn eq(&self, other: &Self) -> bool {
        self.ipa_start == other.ipa_start && self.pa_start == other.pa_start && self.length == other.length && self.flags == other.flags
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct VmMemoryConfig {
    pub regions: Vec<VmMemoryRegion, MAX_VM_MEMORY_REGION>,
}

impl VmMemoryConfig {
    pub const fn default() -> VmMemoryConfig {
        VmMemoryConfig {
            regions: Vec::new(),
        }
    }

    pub fn add_memory_region(&mut self, region: VmMemoryRegion) {
        self.regions.push(region).unwrap();
    }
}
