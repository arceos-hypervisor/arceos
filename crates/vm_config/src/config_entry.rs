use super::cpu_config::VmCpuConfig;
use super::emulated_dev_config::VmEmulatedDeviceConfigList;
use super::image_config::VmImageConfig;
use super::memory_config::VmMemoryConfig;
use super::passthrough_dev_config::VmPassthroughDeviceConfigList;
use crate::NAME_MAX_LENGTH;
use heapless::String;

#[derive(Clone, Debug)]
pub struct VmConfigEntry {
    /// vm id
    pub id: usize,
    /// vm name
    pub name: Option<String<NAME_MAX_LENGTH>>,
    /// vm cmd
    pub cmdline: String<NAME_MAX_LENGTH>,
    /// vm image
    pub image: VmImageConfig,
    /// vm memory
    pub memory: VmMemoryConfig,
    /// vm cpu info
    pub cpu: VmCpuConfig,
    /// vm emulated device
    pub vm_emu_dev_config_list: VmEmulatedDeviceConfigList,
    // vm passthrough device
    pub vm_passthrough_dev_config_list: VmPassthroughDeviceConfigList,
}

impl VmConfigEntry {
    pub fn new(
        id:usize, 
        name:Option<String<NAME_MAX_LENGTH>>, 
        cmd: String<NAME_MAX_LENGTH>,
        image: VmImageConfig,
        memory: VmMemoryConfig,
        cpu: VmCpuConfig,
        vm_emu_dev_config: VmEmulatedDeviceConfigList,
        vm_passthrough_dev_config: VmPassthroughDeviceConfigList
    ) -> Self {
        Self {
            id: id,
            name: name,
            cmdline: cmd,
            image: image,
            memory: memory,
            cpu: cpu,
            vm_emu_dev_config_list: vm_emu_dev_config,
            vm_passthrough_dev_config_list: vm_passthrough_dev_config,
        }
    }
}
