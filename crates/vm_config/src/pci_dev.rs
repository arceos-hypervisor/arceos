use spin::RwLock;
use alloc::collections::BTreeMap;
use core::mem;
use core::slice;
use alloc::sync::Arc;

pub trait ReadWriteStruct {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8];
    fn write_bytes(&mut self, start: usize, bytes: &[u8]);
    fn struct_size() -> u8;
}

pub fn read_u8<T: ReadWriteStruct>(read_struct: &T, start: usize) -> u8 {
    let slice = read_struct.read_bytes(start, 1);
    slice[0]
}

pub fn read_u16<T: ReadWriteStruct>(read_struct: &T, start: usize) -> u16 {
    let slice = read_struct.read_bytes(start, 2);
    u16::from_le_bytes([slice[0], slice[1]])
}

pub fn read_u32<T: ReadWriteStruct>(read_struct: &T, start: usize) -> u32 {
    let slice = read_struct.read_bytes(start, 4);
    u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

pub fn write_u8<T: ReadWriteStruct>(write_struct: &mut T, start: usize, value: u8) {
    write_struct.write_bytes(start, &[value]);
}

pub fn write_u16<T: ReadWriteStruct>(write_struct: &mut T, start: usize, value: u16) {
    write_struct.write_bytes(start, &value.to_le_bytes());
}

pub fn write_u32<T: ReadWriteStruct>(write_struct: &mut T, start: usize, value: u32) {
    write_struct.write_bytes(start, &value.to_le_bytes());
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct PCIDevice {
    pub vendor_id: u16, // 0x0
    pub device_id: u16, // 0x2
    pub command: u16,   // 0x4
    pub status: u16,    // 0x6
    pub revision_id_class_code: [u8; 4], // 0x8
    pub cacheline_size: u8, // 0xc
    pub latency_timer: u8,  // 0xd
    pub header_type: u8,    // 0xe
    pub bist: u8,   // 0xf
    pub bar: [u32; 6],  // 0x10
    pub cardbus_cis_pointer: u32,   // 0x28
    pub subsystem_vendor_id: u16,   // 0x2c
    pub subsystem_id: u16,  // 0x2e
    pub expansion_rom_base_address: u32,    // 0x30
    pub capabilities_pointer: u16,  // 0x34
    pub _reserved1: u16,    // 0x36
    pub _reserved2: u32,    // 0x38
    pub interrupt_line: u8, // 0x3c
    pub interrupt_pin: u8,  // 0x3d
    pub min_gnt: u8,    // 0x3e
    pub max_lat: u8,    // 0x3f
    pub capabilities: Arc<RwLock<BTreeMap<(u8, u8), CapabilityEnum>>>, // the map is (key: region, value: CapabilityEnum)
    // pub caps_start: u16,
    // pub num_caps: u16,
    // pub num_msi_vectors: u8,
    // pub msi_64bits: u8,
    pub bar_size: [u32; 6],
    pub expansion_rom_base_address_size: u32,
    pub num_msix_vectors: u16,
    pub msix_region_size: u16,
    pub msix_address: u64,

    pub bus: u32,
    pub slot: u32,
    pub func: u32,
}

impl ReadWriteStruct for PCIDevice {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8] {
        assert!(start + count <= PCIDevice::struct_size() as usize);
        let device_ptr = self as *const _ as *const u8;
        let slice = unsafe { slice::from_raw_parts(device_ptr.add(start), count) };
        slice
    }

    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        assert!(start + bytes.len() <= PCIDevice::struct_size() as usize);
        let device_ptr = self as *mut _ as *mut u8;
        let slice = unsafe { slice::from_raw_parts_mut(device_ptr.add(start), bytes.len()) };
        slice.copy_from_slice(bytes);
    }

    fn struct_size() -> u8 {
        0xff    // 256 bytes for pci config space
    }
}

impl PCIDevice {
    // readonly fields???
    pub fn find_capability(&self, value: u8) -> Option<CapabilityEnum> {
        if value==0x98 {
            info!("[find_capability]1111111111111111111");
        }
        let range_map = self.capabilities.read();
        if value==0x98 {
            info!("[find_capability]22222222222222222");
        }
        for ((start, end), capability) in range_map.iter() {
            if self.bus==0x0 && self.slot==0x3 && self.func==0x0 {
                info!("[find_capability] start:{:#x} end:{:#x} capability:{:?}", start, end, capability);
            }
            if *start <= value && value < *end {
                return Some(*capability);
            }
        }
        None
    }
    // suppose the capability exists when call this func
    pub fn find_capability_start(&self, offset: u8) -> u8 {
        let range_map = self.capabilities.read();
        for ((start, end), _) in range_map.iter() {
            if *start <= offset && offset < *end {
                return *start;
            }
        }
        return 0xff;
    }
    
}

#[derive(Clone, Debug, Copy)]
pub enum CapabilityEnum {
    CapabilityMsix(CapabilityMsix),
    CapabilityMsi(CapabilityMsi),
    CapabilityDummy(CapabilityDummy),
}

impl ReadWriteStruct for CapabilityEnum {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8] {
        match self {
            CapabilityEnum::CapabilityMsix(cap) => cap.read_bytes(start, count),
            CapabilityEnum::CapabilityMsi(cap) => cap.read_bytes(start, count),
            CapabilityEnum::CapabilityDummy(cap) => cap.read_bytes(start, count),
        }
    }

    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        match self {
            CapabilityEnum::CapabilityMsix(cap) => cap.write_bytes(start, bytes),
            CapabilityEnum::CapabilityMsi(cap) => cap.write_bytes(start, bytes),
            CapabilityEnum::CapabilityDummy(cap) => cap.write_bytes(start, bytes),
        }
    }

    fn struct_size() -> u8 {
        0
    }
}

#[derive(Clone, Debug, Copy, serde::Deserialize)]
#[repr(C)]
pub struct CapabilityMsix {
    pub id: u8,     // 0x0
    pub next_region: u8,  // 0x1 byte address in the config space
    pub message_control: u16,   // 0x2
    pub table: u32, // 0x4
    pub pba: u32,   // 0x8
}

impl ReadWriteStruct for CapabilityMsix {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8] {
        assert!(start + count <= CapabilityMsix::struct_size() as usize);
        let device_ptr = self as *const _ as *const u8;
        let slice = unsafe { slice::from_raw_parts(device_ptr.add(start), count) };
        slice
    }
    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        assert!(start + bytes.len() <= CapabilityMsix::struct_size() as usize);
        let device_ptr = self as *mut _ as *mut u8;
        let slice = unsafe { slice::from_raw_parts_mut(device_ptr.add(start), bytes.len()) };
        slice.copy_from_slice(bytes);
    }
    fn struct_size() -> u8 {
        mem::size_of::<Self>() as u8
    }
}


#[derive(Clone, Debug, Copy, serde::Deserialize)]
#[repr(C)]
pub struct CapabilityMsi {
    pub id: u8,     // 0x0
    pub next_region: u8,  // 0x1 byte address in the config space
    pub message_control: u16,   // 0x2
    pub message_address: u32, // 0x4
    // pub pba: u32,   // 0x8
}

impl ReadWriteStruct for CapabilityMsi {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8] {
        assert!(start + count <= CapabilityMsi::struct_size() as usize);
        let device_ptr = self as *const _ as *const u8;
        let slice = unsafe { slice::from_raw_parts(device_ptr.add(start), count) };
        slice
    }
    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        assert!(start + bytes.len() <= CapabilityMsi::struct_size() as usize);
        let device_ptr = self as *mut _ as *mut u8;
        let slice = unsafe { slice::from_raw_parts_mut(device_ptr.add(start), bytes.len()) };
        slice.copy_from_slice(bytes);
    }
    fn struct_size() -> u8 {
        mem::size_of::<Self>() as u8
    }
}

#[derive(Clone, Debug, Copy, serde::Deserialize)]
#[repr(C)]
pub struct CapabilityDummy {
    pub id: u8,     // 0x0
    pub next_region: u8,  // 0x1 byte address in the config space
    pub control: u16,   // 0x2  maybe this is control
    pub unknown: u32, // 0x4
    // TODO: do not know what the struct is
}

impl ReadWriteStruct for CapabilityDummy {
    fn read_bytes(&self, start: usize, count: usize) -> &[u8] {
        assert!(start + count <= CapabilityDummy::struct_size() as usize);
        let device_ptr = self as *const _ as *const u8;
        let slice = unsafe { slice::from_raw_parts(device_ptr.add(start), count) };
        slice
    }

    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        assert!(start + bytes.len() <= CapabilityDummy::struct_size() as usize);
        let device_ptr = self as *mut _ as *mut u8;
        let slice = unsafe { slice::from_raw_parts_mut(device_ptr.add(start), bytes.len()) };
        slice.copy_from_slice(bytes);
    }
    fn struct_size() -> u8 {
        mem::size_of::<Self>() as u8
    }
}

