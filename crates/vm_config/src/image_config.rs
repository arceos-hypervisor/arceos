use crate::{from_hex, NAME_MAX_LENGTH};
use heapless::String;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct VmImageConfig {
    pub kernel_img_name: Option<String<NAME_MAX_LENGTH>>,
    #[serde(deserialize_with = "from_hex")]
    pub kernel_load_ipa: usize,
    #[serde(deserialize_with = "from_hex")]
    pub kernel_load_pa: usize,
    #[serde(deserialize_with = "from_hex")]
    pub bios_paddr: usize,
    #[serde(deserialize_with = "from_hex")]
    pub bios_entry: usize,
    #[serde(deserialize_with = "from_hex")]
    pub bios_size: usize,
}

impl VmImageConfig {
    pub const fn default() -> VmImageConfig {
        VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa: 0,
            kernel_load_pa: 0,
            bios_paddr: 0,
            bios_entry: 0,
            bios_size: 0,
        }
    }
    pub fn new(
        kernel_img_name: Option<String<NAME_MAX_LENGTH>>,
        kernel_load_ipa: usize, 
        kernel_load_pa: usize, 
        bios_paddr: usize,
        bios_entry: usize,
        bios_size: usize,
    ) -> VmImageConfig {
        VmImageConfig {
            kernel_img_name: kernel_img_name,
            kernel_load_ipa: kernel_load_ipa,
            kernel_load_pa: kernel_load_pa,
            bios_paddr: bios_paddr,
            bios_entry: bios_entry,
            bios_size: bios_size,
        }
    }
}
