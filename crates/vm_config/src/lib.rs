#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

extern crate heapless;
extern crate serde;
extern crate serde_json_core;
extern crate spin;

pub mod config_entry;
pub mod cpu_config;
pub mod emulated_dev_config;
pub mod image_config;
pub mod memory_config;
pub mod passthrough_dev_config;
pub mod pci_dev;

use alloc::sync::Arc;
use spin::RwLock;
use alloc::collections::BTreeMap;

pub use config_entry::VmConfigEntry;

pub use emulated_dev_config::{VmEmulatedDeviceConfig, VmEmulatedDeviceConfigList, EmuDeviceType, DeviceType};
pub use pci_dev::{
    PCIDevice, CapabilityEnum,
    CapabilityDummy, CapabilityMsix, CapabilityMsi, CapabilityPcie, Capability9,
};
pub use passthrough_dev_config::{
    VmPassthroughDeviceConfig, VmPassthroughDeviceConfigList, 
    PortDevice, PassthroughDeviceType, 
    PCI_TYPE_DEVICE};
use memory_config::VmMemoryRegion;

use core::fmt;
use heapless::{String, Vec};
use serde::de::{self, Deserializer, Visitor};

use crate::pci_dev::ReadWriteStruct;

const NAME_MAX_LENGTH: usize = 128;
pub const MAX_BASE_CNT: usize = 4;

/*
#[derive(Clone, serde::Deserialize)]
pub struct VmConfigTable {
    pub vm_num: usize,
    pub entries: [VmConfigEntry; 8],
}
*/

const READ: usize = 1 << 0;
const WRITE: usize = 1 << 1;
const EXECUTE: usize = 1 << 2;
const USER: usize = 1 << 3;
const DEVICE: usize = 1 << 4;

/* 
pub fn deserialize_vm_config_entry(config_json: &'static str) -> Option<VmConfigEntry> {
    let config: Result<(VmConfigEntry, usize), serde_json_core::de::Error> =
        serde_json_core::de::from_str(config_json);
    debug!("this is config:{:?}", config);
    match config {
        Ok(config) => Some(config.0),
        Err(e) => {
            debug!("deserialize_vm_config_entry error: {:?}", e);
            None
        }
    }
}
*/

pub fn create_default_vm_config_entry() -> VmConfigEntry {
    debug!("create_default_vm_config_entry");
    // vm info
    let id = 0;
    let mut name: String<NAME_MAX_LENGTH> = String::new();
    name.push_str("vm1").unwrap();
    let mut cmd: String<NAME_MAX_LENGTH> = String::new();
    cmd.push_str("console=uart8250,io,0x3f8,115200n8 debug\0")
        .unwrap();

    // kernel image
    let mut image_name: String<NAME_MAX_LENGTH> = String::new();
    image_name.push_str("linux").unwrap();
    // kernel load ipa is equal to load pa?
    let image = image_config::VmImageConfig{
        kernel_img_name: Some(image_name),
        kernel_load_ipa: 0x7020_0000,
        kernel_load_pa: 0x7020_0000,
        bios_paddr: 0x400_0000,
        bios_entry: 0x7c00,
        bios_size: 0x2000,
    };

    // memory
    let mut memory = memory_config::VmMemoryConfig::default();
    // physical addr
    let memory_ipa_start = 0x0;
    let memory_pa_start = 0x0; //offset??
    let length = 0x100_0000;
    let flags = READ | WRITE | EXECUTE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // Low RAM2
    let memory_ipa_start = 0x100_0000;
    let memory_pa_start = 0x6100_0000; //offset??
    let length = 0xf00_0000;
    let flags = READ | WRITE | EXECUTE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // RAM
    let memory_ipa_start = 0x7000_0000;
    let memory_pa_start = 0x7000_0000; //offset??
    let length = 0x1000_0000;
    let flags = READ | WRITE | EXECUTE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // pci
    let memory_ipa_start = 0x8000_0000;
    let memory_pa_start = 0x8000_0000; //offset??
    let length = 0x1000_0000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // virtio-9p
    let memory_ipa_start = 0xfe00_0000;
    let memory_pa_start = 0xfe00_0000; //offset??
    let length = 0x1_0000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // 0000:00:01.0??
    let memory_ipa_start = 0xfeb0_0000;
    let memory_pa_start = 0xfeb0_0000; //offset??
    let length = 0x10_0000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // IO APIC
    let memory_ipa_start = 0xfec0_0000;
    let memory_pa_start = 0xfec0_0000; //offset??
    let length = 0x1000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // HPET
    let memory_ipa_start = 0xfed0_0000;
    let memory_pa_start = 0xfed0_0000; //offset??
    let length = 0x1000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);
    // Local APIC
    let memory_ipa_start = 0xfee0_0000;
    let memory_pa_start = 0xfee0_0000; //offset??
    let length = 0x1000;
    let flags = READ | WRITE | DEVICE;
    let memory_region = VmMemoryRegion::new(memory_ipa_start, memory_pa_start, length, flags);
    memory.add_memory_region(memory_region);


    // cpu
    let cpu = cpu_config::VmCpuConfig::new(1, 1, 0);

    // emulated device
    let mut vm_emu_dev_config_list = VmEmulatedDeviceConfigList::default();
    // 0x20 and 0xa0 are ports about pic
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x20).unwrap();
    base.push(0xa0).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x2).unwrap();
    range.push(0x2).unwrap();
    let pic = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDevicePIC,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(pic);
    // 0xcf8: pci config space
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0xcf8).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x8).unwrap();
    let pci_config_space = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDevicePCI,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(pci_config_space);
    // 0x80 is a port about debug
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x80).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let debug_port = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDebugPort,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(debug_port);
    // COM2-COM4
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x2f8).unwrap();
    base.push(0x3e8).unwrap();
    base.push(0x2e8).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x8).unwrap();
    range.push(0x8).unwrap();
    range.push(0x8).unwrap();
    let uart16550 = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceUart16550,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(uart16550);
    // 0x40-0x47: PIT
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x40).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x8).unwrap();
    let pit = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDevicePit,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(pit);
    // 0x70-0x71: CMOS
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x70).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x2).unwrap();
    let cmos = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceCmos,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(cmos);
    // 0xf0 and 0xf1 are ports about fpu
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0xf0).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x2).unwrap();
    let fpu_dummy = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(fpu_dummy);
    // 0x3d4 and 0x3d5 are ports about vga
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x3d4).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x2).unwrap();
    let vga_dummy = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(vga_dummy);
    // 0x87 is a port about dma
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x87).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let dma_dummy = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(dma_dummy);
    // 0x60 and 0x64 are ports about ps/2 controller
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x60).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let ps2_dummy1 = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(ps2_dummy1);
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x64).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let ps2_dummy2 = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Pio,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(ps2_dummy2);
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x800).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x40).unwrap();
    let virtual_local_apic = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceVirtialLocalApic,
        device_type: DeviceType::Msr,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(virtual_local_apic);
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0x1b).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let apic = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceVirtialLocalApic,
        device_type: DeviceType::Msr,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(apic);
    // linux read this amd-related msr on my intel cpu for some unknown reason
    let mut base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    base.push(0xc0011029).unwrap();
    let mut range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    range.push(0x1).unwrap();
    let dummy_msr = VmEmulatedDeviceConfig {
        name: None,
        emu_type: EmuDeviceType::EmuDeviceDummy,
        device_type: DeviceType::Msr,
        base: base,
        range: range,
    };
    vm_emu_dev_config_list.add_device_config(dummy_msr);

    // passthrough device
    // COM1
    let mut passthrough_dev_config_list = VmPassthroughDeviceConfigList::default();
    let mut msr_base: Vec<usize, MAX_BASE_CNT> = Vec::new();
    msr_base.push(0x3f8).unwrap();
    let mut msr_range: Vec<usize, MAX_BASE_CNT> = Vec::new();
    msr_range.push(0x8).unwrap();
    let com1 = PortDevice {
        base: msr_base,
        range: msr_range,
    };
    let passthrough_com1 = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PORT,
        None,
        None,
        Some(com1),
    );
    passthrough_dev_config_list.add_device_config(passthrough_com1);
    // unknown: 00:00:00
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    let pci_unknown = PCIDevice {
        vendor_id: 0x8086,  
        device_id: 0x29c0,
        command: 0x103,
        status: 0x0,
        revision_id_class_code: [0, 0, 0, 0x6],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x0,
        bist: 0x0,
        bar: [0x0; 6],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x1100,
        expansion_rom_base_address: 0x0,
        capabilities_pointer: 0x0,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0x0,
        interrupt_pin: 0x0,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size:[0x0; 6],
        expansion_rom_base_address_size: 0x0,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x0,
        func: 0x0,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // unknown: 00:01:00
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    let pci_unknown = PCIDevice {
        vendor_id: 0x1234,  
        device_id: 0x1111,
        command: 0x103,
        status: 0x0,
        revision_id_class_code: [0x2, 0, 0, 0x3],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x0,
        bist: 0x0,
        bar: [0xfd000008, 0x0, 0xfebd4000, 0x0, 0x0, 0x0],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x1100,
        expansion_rom_base_address: 0xfebc0000,
        capabilities_pointer: 0x0,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0x0,
        interrupt_pin: 0x0,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0xff000008, 0x0, 0xfffff000, 0x0, 0x0, 0x0],
        expansion_rom_base_address_size: 0xffff0000,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x1,
        func: 0x0,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // unknown: 00:02:00
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    capabilities.write().insert(
        (0xc8, 0xc8+ CapabilityDummy::struct_size()), 
        CapabilityEnum::CapabilityDummy(
            CapabilityDummy {
                id: 0x01,
                next_region: 0xd0,
                unknown1: [0x0, 0x0],
                unknown2: [0x0, 0x0, 0x0, 0x0],
            }
        ),
    );
    capabilities.write().insert(
        (0xd0, 0xd0 + CapabilityMsi::struct_size()), 
        CapabilityEnum::CapabilityMsi(
            CapabilityMsi {
                id: 0x05,
                next_region: 0xe0,
                message_control: 0x80,
                message_address: 0x0,
                message_upper_address: 0x0,
                message_data: 0x0,
            }
        ),
    );
    capabilities.write().insert(
        (0xe0, 0xe0 + CapabilityDummy::struct_size()), 
        CapabilityEnum::CapabilityPcie(
            CapabilityPcie {
                id: 0x10,
                next_region: 0xa0,
                control: 0x91,
                unknown1: 0x8000,
                unknown2: 0x0,
            }
        ),
    );
    capabilities.write().insert(
        (0xa0, 0xa0 + CapabilityMsix::struct_size()), 
        CapabilityEnum::CapabilityMsix(
            CapabilityMsix {
                id: 0x11,
                next_region: 0x0,
                message_control: 0x4,
                table: 0x0,
                pba: 0x0,
            }
        ),
    );
    let pci_unknown = PCIDevice {
        vendor_id: 0x8086,  
        device_id: 0x10d3,
        command: 0x103,
        status: 0x10,
        revision_id_class_code: [0x0, 0, 0, 0x2],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x0,
        bist: 0x0,
        bar: [0xfeb80000, 0xfeba0000, 0xc0c1, 0xfebd0000, 0x0, 0x0],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x8086,
        subsystem_id: 0x0,
        expansion_rom_base_address: 0xfeb40000,
        capabilities_pointer: 0xc8,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0xb,
        interrupt_pin: 0x1,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0xfffe0000, 0xfffe0000, 0xffffffe1, 0xffffc000, 0x0, 0x0],
        expansion_rom_base_address_size: 0xfffc0000,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x2,
        func: 0x0,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // unknown: 00:1f:00
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    let pci_unknown = PCIDevice {
        vendor_id: 0x8086,  
        device_id: 0x2918,
        command: 0x103,
        status: 0x0,
        revision_id_class_code: [0x2, 0, 0x1, 0x6],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x80,
        bist: 0x0,
        bar: [0x0; 6],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x1100,
        expansion_rom_base_address: 0x0,
        capabilities_pointer: 0x0,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0x0,
        interrupt_pin: 0x0,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0x0; 6],
        expansion_rom_base_address_size: 0x0,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x1f,
        func: 0x0,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // unknown: 00:1f:02
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    capabilities.write().insert(
        (0x80, 0x80 + CapabilityMsi::struct_size()), 
        CapabilityEnum::CapabilityMsi(
            CapabilityMsi {
                id: 0x05,
                next_region: 0xa8,
                message_control: 0x80,
                message_address: 0xfee01004,
                message_upper_address: 0x0,
                message_data: 0x0,
            }
        ),
    );
    capabilities.write().insert(
        (0xa8, 0xa8 + CapabilityDummy::struct_size()), 
        CapabilityEnum::CapabilityDummy(
            CapabilityDummy {
                id: 0x12,
                next_region: 0x0,
                unknown1: [0x0, 0x0],
                unknown2: [0x0, 0x0, 0x0, 0x0],
            }
        ),
    );
    let pci_unknown = PCIDevice {
        vendor_id: 0x8086,  
        device_id: 0x2922,
        command: 0x107,
        status: 0x10,
        revision_id_class_code: [0x2, 0x1, 0x6, 0x1],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x80,
        bist: 0x0,
        bar: [0x0, 0x0, 0x0, 0x0, 0xc0e1, 0xfebd6000],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x1100,
        expansion_rom_base_address: 0x0,
        capabilities_pointer: 0x80,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0xa,
        interrupt_pin: 0x1,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0x0, 0x0, 0x0, 0x0, 0xffffffe1, 0xfffff000],
        expansion_rom_base_address_size: 0x0,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x1f,
        func: 0x2,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // unknown: 00:1f:03
    let capabilities:RwLock<BTreeMap<(u8, u8), CapabilityEnum>> = RwLock::new(BTreeMap::new());
    let pci_unknown = PCIDevice {
        vendor_id: 0x8086,  
        device_id: 0x2930,
        command: 0x103,
        status: 0x0,
        revision_id_class_code: [0x2, 0, 0x5, 0xc],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x80,
        bist: 0x0,
        bar: [0x0, 0x0, 0x0, 0x0, 0x701, 0x0],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x1100,
        expansion_rom_base_address: 0x0,
        capabilities_pointer: 0x0,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0x1,
        interrupt_pin: 0xa,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0x0, 0x0, 0x0, 0x0, 0xffffffc1, 0x0],
        expansion_rom_base_address_size: 0x0,
        num_msix_vectors: 0,
        msix_region_size: 0x0,
        msix_address: 0x0,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x1f,
        func: 0x3,
    };
    let passthrough_pci_unknown = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_unknown),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_unknown);
    // Virtio-9p 00:03:00
    let capabilities: RwLock<BTreeMap<(u8, u8), CapabilityEnum>> =  RwLock::new(BTreeMap::new());
    capabilities.write().insert(
        (0x98, 0x98 + CapabilityMsix::struct_size()), 
        CapabilityEnum::CapabilityMsix(
            CapabilityMsix {
                id: 0x11,
                next_region: 0x84,
                message_control: 0x01,
                table: 0x1,
                pba: 0x0,
            }
        ),
    );
    capabilities.write().insert(
        (0x84, 0x84 + Capability9::struct_size()), 
        CapabilityEnum::Capability9(
            Capability9 {
                id: 0x09,
                next_region: 0x70,
                unknown1: [0x0, 0x5],
                unknown2: [0x0, 0x0, 0x0, 0x0],
                unknown3: 0x0,
                unknown4: 0x0,
            }
        ),
    );
    capabilities.write().insert(
        (0x70, 0x70 + Capability9::struct_size()), 
        CapabilityEnum::Capability9(
            Capability9 {
                id: 0x09,
                next_region: 0x60,
                unknown1: [0x0, 0x2],
                unknown2: [0x4, 0x0, 0x0, 0x0],
                unknown3: 0x3000,
                unknown4: 0x1000,
            }
        ),
    );
    capabilities.write().insert(
        (0x60, 0x60 + Capability9::struct_size()), 
        CapabilityEnum::Capability9(
            Capability9 {
                id: 0x09,
                next_region: 0x50,
                unknown1: [0x0, 0x4],
                unknown2: [0x4, 0x0, 0x0, 0x0],
                unknown3: 0x2000,
                unknown4: 0x1000,
            }
        ),
    );
    capabilities.write().insert(
        (0x50, 0x50 + Capability9::struct_size()), 
        CapabilityEnum::Capability9(
            Capability9 {
                id: 0x09,
                next_region: 0x40,
                unknown1: [0x0, 0x3],
                unknown2: [0x4, 0x0, 0x0, 0x0],
                unknown3: 0x1000,
                unknown4: 0x1000,
            }
        ),
    );
    capabilities.write().insert(
        (0x40, 0x40 + Capability9::struct_size()), 
        CapabilityEnum::Capability9(
            Capability9 {
                id: 0x09,
                next_region: 0x0,
                unknown1: [0x0, 0x1],
                unknown2: [0x4, 0x0, 0x0, 0x0],
                unknown3: 0x0,
                unknown4: 0x1000,
            }
        ),
    );
    let pci_virtio = PCIDevice {
        vendor_id: 0x1af4,  
        device_id: 0x1001,
        command: 0x107,
        status: 0x10,
        revision_id_class_code: [0x0, 0, 0, 0x1],
        cacheline_size: 0x0,
        latency_timer: 0x0,
        header_type: 0x0,
        bist: 0x0,
        bar: [0xc001, 0xfebd5000, 0x0, 0x0, 0xfe00000c, 0x0],
        cardbus_cis_pointer: 0x0,
        subsystem_vendor_id: 0x1af4,
        subsystem_id: 0x2,
        expansion_rom_base_address: 0x0,
        capabilities_pointer: 0x98,
        _reserved1: 0x0,
        _reserved2: 0x0,
        interrupt_line: 0xb,
        interrupt_pin: 0x1,
        min_gnt: 0x0,
        max_lat: 0x0,
        capabilities: Arc::new(capabilities),
        
        bar_size: [0xffffff81, 0xfffff000, 0x0, 0x0, 0xffffc00c, 0xffffffff],
        expansion_rom_base_address_size: 0x0,
        num_msix_vectors: 2,
        msix_region_size: 0x1000,
        msix_address: 0xfebda000,
        // device_type: PCI_TYPE_DEVICE,
        
        bus: 0,
        slot: 0x3,
        func: 0x0,
    };
    pci_virtio.find_capability(0x98);
    let passthrough_pci_virtio = VmPassthroughDeviceConfig::new(
        PassthroughDeviceType::PCI,
        Some(pci_virtio),
        None,
        None,
    );
    passthrough_dev_config_list.add_device_config(passthrough_pci_virtio);

    // create vm config entry
    VmConfigEntry{
        id: id,
        name: Some(name),
        cmdline: cmd,
        image: image,
        memory: memory,
        cpu: cpu,
        vm_emu_dev_config_list: vm_emu_dev_config_list,
        vm_passthrough_dev_config_list: passthrough_dev_config_list,
    }
}
struct HexVisitor;

impl<'de> Visitor<'de> for HexVisitor {
    type Value = usize;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex string")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        let value = value.strip_prefix("0x").unwrap_or(value);
        let ret = usize::from_str_radix(value, 16);

        ret.map_err(E::custom)
    }
}

pub fn from_hex<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = deserializer.deserialize_str(HexVisitor);
    ret
}

/* 
        let json_str = r#"{
            "id": 0,
            "name": "test_vm",
            "cmdline": "test_cmdline",
            "image": {
                "kernel_img_name": "test_kernel",
                "kernel_load_ipa": "0x1000",
                "kernel_load_pa": "0x2000",
                "kernel_entry_point": "0x3000",
                "device_tree_load_ipa": "0x4000",
                "ramdisk_load_ipa": "0x5000"
            },
            "memory": {
                "region": [
                    {
                        "ipa_start": "0x1000",
                        "pa_start": "0x2000",
                        "length": "0x3000"
                    },
                    {
                        "ipa_start": "0x4000",
                        "pa_start": "0x5000",
                        "length": "0x6000"
                    }
                ]
            },
            "cpu": {
                "num": 4,
                "allocate_bitmap": "0xf",
                "master": 0
            },
            "vm_emu_dev_config": {
                "emu_dev_list": [
                    {
                        "name": "test_dev",
                        "base_ipa": "0x1000",
                        "length": "0x1000",
                        "irq_id": 1,
                        "cfg_list": [4, 5, 6],
                        "emu_type": "console"
                    },
                    {
                        "name": "test_dev1",
                        "base_ipa": "0x2000",
                        "length": "0x1000",
                        "irq_id": 2,
                        "cfg_list": [1, 2, 3],
                        "emu_type": "virtio_blk"
                    },
                    {
                        "name": "test_dev2",
                        "base_ipa": "0x3000",
                        "length": "0x1000",
                        "irq_id": 3,
                        "cfg_list": [7, 8, 9],
                        "emu_type": "virtio_net"
                    }
                ]
            },
            "vm_passthrough_dev_config": {
                "regions": [
                    {
                        "ipa": "0x100",
                        "pa": "0x200",
                        "length": "0x300"
                    },
                    {
                        "ipa": "0x400",
                        "pa": "0x500",
                        "length": "0x600"
                    }
                ],
                "irqs": [1, 2, 3],
                "streams_ids": [4, 5, 6]
            }
        }"#;
        let config:Option<vm_config::VmConfigEntry> = vm_config::deserialize_vm_config_entry(json_str);
*/