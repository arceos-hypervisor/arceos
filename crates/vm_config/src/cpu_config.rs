use crate::from_hex;

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct VmCpuConfig {
    pub num: usize,
    #[serde(deserialize_with = "from_hex")]
    pub allocate_bitmap: usize,
    pub master: i32,
}

impl VmCpuConfig {
    pub const fn default() -> VmCpuConfig {
        VmCpuConfig {
            num: 0,
            allocate_bitmap: 0,
            master: 0,
        }
    }
    pub fn new(
        num: usize,
        allocate_bitmap: usize,
        master: i32,
    ) -> Self {
        Self {
            num: num,
            allocate_bitmap: allocate_bitmap,
            master: master,
        }
    }
}