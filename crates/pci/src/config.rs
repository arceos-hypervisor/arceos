use crate::msix::{Msix, MSIX_TABLE_ENTRY_SIZE};
use crate::util::byte_code::ByteCode;
use crate::util::num_ops::ranges_overlap;
use crate::{
    le_read_u16, le_read_u32, le_read_u64, le_write_u16, le_write_u32, le_write_u64,
    pci_ext_cap_next, PciBus,
};
use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::Range;
use core::ptr::{read, write};
use hashbrown::HashMap;
use hypercraft::{HyperError, HyperResult as Result, PciError};
use hypercraft::{HyperResult, MmioOps, PioOps, RegionOps};
use lazy_static::lazy_static;
use spin::Mutex;

/// The mem64 base of emulated PCI devices bars.
pub const PCI_EMUL_MEMBASE64: u64 = 0x4000_0000_0000; /* 256GB */
/// The mem64 limit of emulated PCI devices bars.
pub const PCI_EMUL_MEMLIMIT64: u64 = 0x8000_0000_0000; /* 512GB */
/// The mem32 base of emulated PCI devices bars.
pub const PCI_EMUL_MEMBASE32: u64 = 0x8000_0000; /* 2GB */
/// The mem32 limit of emulated PCI devices bars.
pub const PCI_EMUL_MEMLIMIT32: u64 = 0xE000_0000; /* 3.5GB */
/// The io base of emulated PCI devices bars.
pub const PCI_EMUL_IOBASE: u64 = 0x2000;
/// The io limit of emulated PCI devices bars.
pub const PCI_EMUL_IOLIMIT: u64 = 0x10000;

/// Size in bytes of the configuration space of legacy PCI device.
pub const PCI_CONFIG_SPACE_SIZE: usize = 256;
/// Size in bytes of the configuration space of PCIe device.
pub const PCIE_CONFIG_SPACE_SIZE: usize = 4096;
/// Size in bytes of dword.
pub const REG_SIZE: usize = 4;
/// Max number of function.
pub const MAX_FUNC: u8 = 8;

/// Vendor ID Register.
pub const VENDOR_ID: u8 = 0x0;
/// Device ID register.
pub const DEVICE_ID: u8 = 0x02;
/// Command register.
pub const COMMAND: u8 = 0x04;
pub const REVISION_ID: usize = 0x08;
pub const CLASS_PI: u16 = 0x09;
/// Sub-Class Code Register.
pub const SUB_CLASS_CODE: u8 = 0x0a;
pub const SUBSYSTEM_VENDOR_ID: usize = 0x2c;
pub const SUBSYSTEM_ID: usize = 0x2e;
/// Header Type register.
pub const HEADER_TYPE: u8 = 0x0e;
/// Base address register 0.
pub const BAR_0: u8 = 0x10;
/// Base address register 5.
pub const BAR_5: u8 = 0x24;
pub const PCI_CAP_VNDR_AND_NEXT_SIZE: u8 = 2;
pub const PCI_CAP_ID_VNDR: u8 = 0x9;
/// Secondary bus number register.
pub const SECONDARY_BUS_NUM: u8 = 0x19;
/// Subordinate bus number register.
pub const SUBORDINATE_BUS_NUM: u8 = 0x1a;
/// I/O base register.
pub const IO_BASE: u8 = 0x1c;
/// Memory base register.
pub const MEMORY_BASE: u8 = 0x20;
/// Prefetchable memory base register.
pub const PREF_MEMORY_BASE: u8 = 0x24;
/// Prefetchable memory limit register.
pub const PREF_MEMORY_LIMIT: u8 = 0x26;
const ROM_ADDRESS_ENDPOINT: usize = 0x30;
const ROM_ADDRESS_BRIDGE: usize = 0x38;

/// 64-bit prefetchable memory addresses.
pub const PREF_MEM_RANGE_64BIT: u8 = 0x01;

/// I/O space enable.
pub const COMMAND_IO_SPACE: u16 = 0x0001;
/// Memory space enable.
pub const COMMAND_MEMORY_SPACE: u16 = 0x0002;

/// Class code of host bridge.
pub const CLASS_CODE_HOST_BRIDGE: u16 = 0x0600;
/// Class code of ISA bridge.
pub const CLASS_CODE_ISA_BRIDGE: u16 = 0x0601;
/// Class code of PCI-to-PCI bridge.
pub const CLASS_CODE_PCI_BRIDGE: u16 = 0x0604;
/// Type 0 configuration Space Header Layout.
pub const HEADER_TYPE_ENDPOINT: u8 = 0x0;
/// Type 1 configuration Space Header Layout.
pub const HEADER_TYPE_BRIDGE: u8 = 0x01;
/// Multi-function device.
pub const HEADER_TYPE_MULTIFUNC: u8 = 0x80;
/// The vendor ID for Red Hat / Qumranet.
pub const PCI_VENDOR_ID_REDHAT_QUMRANET: u16 = 0x1af4;
/// The vendor ID for PCI devices other than virtio.
pub const PCI_VENDOR_ID_REDHAT: u16 = 0x1b36;
/// The sub device ID for Red Hat / Qumranet.
pub const PCI_SUBDEVICE_ID_QEMU: u16 = 0x1100;

const PCI_CONFIG_HEAD_END: u8 = 64;
const NEXT_CAP_OFFSET: u8 = 0x01;
const STATUS_CAP_LIST: u16 = 0x0010;

/// 16 bits PCI Status.
pub const STATUS: u8 = 0x06;
/// PCI Interrupt Status.
pub const STATUS_INTERRUPT: u8 = 0x08;
const CACHE_LINE_SIZE: u8 = 0x0c;
const PRIMARY_BUS_NUM: u8 = 0x18;
const IO_LIMIT: u8 = 0x1d;
const PREF_MEM_BASE_UPPER: u8 = 0x28;
const CAP_LIST: u8 = 0x34;
const INTERRUPT_LINE: u8 = 0x3c;
pub const INTERRUPT_PIN: u8 = 0x3d;
pub const BRIDGE_CONTROL: u8 = 0x3e;

const BRIDGE_CTL_PARITY_ENABLE: u16 = 0x0001;
const BRIDGE_CTL_SERR_ENABLE: u16 = 0x0002;
const BRIDGE_CTL_ISA_ENABLE: u16 = 0x0004;
const BRIDGE_CTL_VGA_ENABLE: u16 = 0x0008;
const BRIDGE_CTL_VGA_16BIT_DEC: u16 = 0x0010;
pub const BRIDGE_CTL_SEC_BUS_RESET: u16 = 0x0040;
const BRIDGE_CTL_FAST_BACK: u16 = 0x0080;
const BRIDGE_CTL_DISCARD_TIMER: u16 = 0x0100;
const BRIDGE_CTL_SEC_DISCARD_TIMER: u16 = 0x0200;
const BRIDGE_CTL_DISCARD_TIMER_STATUS: u16 = 0x0400;
const BRIDGE_CTL_DISCARD_TIMER_SERR_E: u16 = 0x0800;

pub const COMMAND_BUS_MASTER: u16 = 0x0004;
const COMMAND_SERR_ENABLE: u16 = 0x0100;
#[cfg(test)]
const COMMAND_FAST_BACK: u16 = 0x0200;
pub const COMMAND_INTERRUPT_DISABLE: u16 = 0x0400;

const STATUS_PARITY_ERROR: u16 = 0x0100;
const STATUS_SIG_TARGET_ABORT: u16 = 0x0800;
const STATUS_RECV_TARGET_ABORT: u16 = 0x1000;
const STATUS_RECV_MASTER_ABORT: u16 = 0x2000;
const STATUS_SIG_SYS_ERROR: u16 = 0x4000;
const STATUS_DETECT_PARITY_ERROR: u16 = 0x8000;

pub const BAR_IO_SPACE: u8 = 0x01;
pub const IO_BASE_ADDR_MASK: u32 = 0xffff_fffc;
pub const MEM_BASE_ADDR_MASK: u64 = 0xffff_ffff_ffff_fff0;
pub const BAR_MEM_64BIT: u8 = 0x04;
const BAR_PREFETCH: u8 = 0x08;
pub const BAR_SPACE_UNMAPPED: u64 = 0xffff_ffff_ffff_ffff;
/// The maximum Bar ID numbers of a Type 0 device
const BAR_NUM_MAX_FOR_ENDPOINT: u8 = 6;
/// The maximum Bar ID numbers of a Type 1 device
const BAR_NUM_MAX_FOR_BRIDGE: u8 = 2;
/// mmio bar's minimum size shall be 4KB
pub const MINIMUM_BAR_SIZE_FOR_MMIO: usize = 0x1000;
/// pio bar's minimum size shall be 4B
const MINIMUM_BAR_SIZE_FOR_PIO: usize = 0x4;

/// PCI Express capability registers, same as kernel defines

const PCI_EXT_CAP_VER_SHIFT: u8 = 16;
const PCI_EXT_CAP_NEXT_SHIFT: u8 = 20;
const PCI_EXP_VER2_SIZEOF: u8 = 0x3c;
const PCI_EXP_FLAGS_VER2: u16 = 0x0002;
const PCI_EXP_FLAGS_SLOT: u16 = 0x0100;
// PCIe type flag
const PCI_EXP_FLAGS_TYPE_SHIFT: u16 = 4;
const PCI_EXP_FLAGS_TYPE: u16 = 0x00f0;

// Role-Based error reporting.
const PCI_EXP_DEVCAP_RBER: u32 = 0x8000;

// Correctable error reporting enable.
const PCI_EXP_DEVCTL_CERE: u16 = 0x01;
// Non-Fatal error reporting enable.
const PCI_EXP_DEVCTL_NFERE: u16 = 0x02;
// Fatal error reporting enable.
const PCI_EXP_DEVCTL_FERE: u16 = 0x04;
// Unsupported request reporting enable.
const PCI_EXP_DEVCTL_URRE: u16 = 0x08;

// Supported max link speed, 16GT for default.
const PCI_EXP_LNKCAP_MLS_16GT: u32 = 0x0000_0004;
// Supported maximum link width, X32 for default.
const PCI_EXP_LNKCAP_MLW_X32: u32 = 0x0000_0200;
// Active state power management support.
const PCI_EXP_LNKCAP_ASPMS_0S: u32 = 0x0000_0400;
// Link bandwidth notification capability.
const PCI_EXP_LNKCAP_LBNC: u32 = 0x0020_0000;
// Data link layer link active reporting capable.
const PCI_EXP_LNKCAP_DLLLARC: u32 = 0x0010_0000;
// Port number reg's shift.
const PCI_EXP_LNKCAP_PN_SHIFT: u8 = 24;

/// Link Training
pub const PCI_EXP_LNKSTA: u16 = 18;
// Current link speed, 2.5GB for default.
pub const PCI_EXP_LNKSTA_CLS_2_5GB: u16 = 0x0001;
// Negotiated link width, X1 for default.
pub const PCI_EXP_LNKSTA_NLW_X1: u16 = 0x0010;
/// Data Link Layer Link Active
pub const PCI_EXP_LNKSTA_DLLLA: u16 = 0x2000;
/// Negotiated Link Width
pub const PCI_EXP_LNKSTA_NLW: u16 = 0x03f0;

// Attention button present.
const PCI_EXP_SLTCAP_ABP: u32 = 0x0000_0001;
// Power controller present.
const PCI_EXP_SLTCAP_PCP: u32 = 0x0000_0002;
// Attention indicator present.
const PCI_EXP_SLTCAP_AIP: u32 = 0x0000_0008;
// Power indicator present.
const PCI_EXP_SLTCAP_PIP: u32 = 0x0000_0010;
// Hot-Plug surprise.
const PCI_EXP_SLTCAP_HPS: u32 = 0x0000_0020;
// Hot-Plug capable.
const PCI_EXP_SLTCAP_HPC: u32 = 0x0000_0040;
// Physical slot number reg's shift.
const PCI_EXP_SLTCAP_PSN_SHIFT: u32 = 19;

/// Slot Control
pub const PCI_EXP_SLTCTL: u16 = 24;
/// Attention Button Pressed Enable
pub const PCI_EXP_SLTCTL_ABPE: u16 = 0x0001;
/// Presence Detect Changed Enable
pub const PCI_EXP_SLTCTL_PDCE: u16 = 0x0008;
/// Command Completed Interrupt Enable
pub const PCI_EXP_SLTCTL_CCIE: u16 = 0x0010;
/// Hot-Plug Interrupt Enable
pub const PCI_EXP_SLTCTL_HPIE: u16 = 0x0020;
// Attention Indicator Control.
const PCI_EXP_SLTCTL_AIC: u16 = 0x00c0;
// Attention Indicator off.
const PCI_EXP_SLTCTL_ATTN_IND_OFF: u16 = 0x00c0;
// Power Indicator Control.
pub(crate) const PCI_EXP_SLTCTL_PIC: u16 = 0x0300;
// Power Indicator blinking.
pub(crate) const PCI_EXP_SLTCTL_PWR_IND_BLINK: u16 = 0x200;
/// Power Indicator on
pub const PCI_EXP_SLTCTL_PWR_IND_ON: u16 = 0x0100;
// Power Indicator off.
pub const PCI_EXP_SLTCTL_PWR_IND_OFF: u16 = 0x0300;
/// Power Controller Control
pub const PCI_EXP_SLTCTL_PCC: u16 = 0x0400;
/// Power Off
pub const PCI_EXP_SLTCTL_PWR_OFF: u16 = 0x0400;
// Electromechanical interlock control.
const PCI_EXP_SLTCTL_EIC: u16 = 0x0800;

/// Slot Status
pub const PCI_EXP_SLTSTA: u16 = 26;
/// Attention Button Pressed
pub const PCI_EXP_SLTSTA_ABP: u16 = 0x0001;
/// Power Fault Detected
const PCI_EXP_SLTSTA_PFD: u16 = 0x0002;
/// MRL Sensor Changed
const PCI_EXP_SLTSTA_MRLSC: u16 = 0x0004;
/// Presence Detect Changed
pub const PCI_EXP_SLTSTA_PDC: u16 = 0x0008;
/// Command Completed
pub const PCI_EXP_SLTSTA_CC: u16 = 0x0010;
/// Presence Detect State
pub const PCI_EXP_SLTSTA_PDS: u16 = 0x0040;
pub const PCI_EXP_SLOTSTA_EVENTS: u16 = PCI_EXP_SLTSTA_ABP
    | PCI_EXP_SLTSTA_PFD
    | PCI_EXP_SLTSTA_MRLSC
    | PCI_EXP_SLTSTA_PDC
    | PCI_EXP_SLTSTA_CC;
pub const PCI_EXP_HP_EV_SPT: u16 = PCI_EXP_SLTCTL_ABPE | PCI_EXP_SLTCTL_PDCE | PCI_EXP_SLTCTL_CCIE;

// System error on correctable error enable.
const PCI_EXP_RTCTL_SECEE: u16 = 0x01;
// System error on non-fatal error enable.
const PCI_EXP_RTCTL_SENFEE: u16 = 0x02;
// System error on fatal error enable.
const PCI_EXP_RTCTL_SEFEE: u16 = 0x04;

// Alternative Routing-ID.
const PCI_EXP_DEVCAP2_ARI: u32 = 0x0000_0020;
// Extended Fmt Field Supported.
const PCI_EXP_DEVCAP2_EFF: u32 = 0x0010_0000;
// End-End TLP Prefix Supported.
const PCI_EXP_DEVCAP2_EETLPP: u32 = 0x0020_0000;
// Alternative Routing-ID.
const PCI_EXP_DEVCTL2_ARI: u16 = 0x0020;
// End-End TLP Prefix Blocking
const PCI_EXP_DEVCTL2_EETLPPB: u16 = 0x8000;

// Supported Link Speeds Vector.
const PCI_EXP_LNKCAP2_SLS_2_5GB: u32 = 0x02;
const PCI_EXP_LNKCAP2_SLS_5_0GB: u32 = 0x04;
const PCI_EXP_LNKCAP2_SLS_8_0GB: u32 = 0x08;
const PCI_EXP_LNKCAP2_SLS_16_0GB: u32 = 0x10;

// Target Link Speed, 16GT for default.
const PCI_EXP_LNKCTL2_TLS_16_0GT: u16 = 0x0004;

/// Hot plug event
/// Presence detect changed
pub const PCI_EXP_HP_EV_PDC: u16 = PCI_EXP_SLTCTL_PDCE;
/// Attention button pressed
pub const PCI_EXP_HP_EV_ABP: u16 = PCI_EXP_SLTCTL_ABPE;
/// Command completed
pub const PCI_EXP_HP_EV_CCI: u16 = PCI_EXP_SLTCTL_CCIE;

// XHCI device id
pub const PCI_DEVICE_ID_REDHAT_XHCI: u16 = 0x000d;

// PvPanic device id
pub const PCI_DEVICE_ID_REDHAT_PVPANIC: u16 = 0x0011;

// Device classes and subclasses
pub const PCI_CLASS_MEMORY_RAM: u16 = 0x0500;
pub const PCI_CLASS_SERIAL_USB: u16 = 0x0c03;
pub const PCI_CLASS_SYSTEM_OTHER: u16 = 0x0880;

// Pci Bar Alloctor for virtual pci devices.
lazy_static! {
    pub static ref PCI_BAR_ALLOCATOR: Arc<Mutex<PciBarAllocator>> =
        Arc::new(Mutex::new(PciBarAllocator::new()));
}

/// Type of bar region.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum RegionType {
    Io,
    Mem32Bit,
    Mem64Bit,
}

impl core::fmt::Display for RegionType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RegionType::Io => "PIO",
                RegionType::Mem32Bit => "32 bits MMIO",
                RegionType::Mem64Bit => "64 bits MMIO",
            }
        )
    }
}

/// Bar allocator trait.
pub trait BarAllocTrait: Sized + Send + Sync + Clone {
    fn alloc(region_type: RegionType, size: u64) -> HyperResult<u64>;
    fn dealloc(region_type: RegionType, addr: u64, size: u64) -> HyperResult<()>;
}

/// Registered bar.
#[derive(Clone)]
pub struct Bar {
    region_type: RegionType,
    address: u64,
    // use host virtual address to get data
    actual_address: u64,
    pub size: u64,
    ops: Option<RegionOps>,
}

impl PioOps for Bar {
    fn port_range(&self) -> core::ops::Range<u16> {
        self.address as u16..(self.address + self.size) as u16
    }

    fn read(&mut self, port: u16, access_size: u8) -> HyperResult<u32> {
        let offset = port - self.address as u16;
        let read_func = &*self.ops.as_ref().unwrap().read;
        let ret = read_func(offset as u64, access_size)?;
        Ok(ret as u32)
    }

    fn write(&mut self, port: u16, access_size: u8, value: u32) -> HyperResult {
        let offset = port - self.address as u16;
        let write_func = &*self.ops.as_ref().unwrap().write;
        let value_bytes = value.to_le_bytes();
        let value_byte: &[u8] = match access_size {
            1 => &value_bytes[0..1],
            2 => &value_bytes[0..2],
            4 => &value_bytes[0..4],
            _ => return Err(HyperError::InValidPioWrite),
        };
        write_func(offset as u64, access_size, value_byte)
    }
}

impl MmioOps for Bar {
    fn mmio_range(&self) -> core::ops::Range<u64> {
        self.address..self.address + self.size
    }

    fn read(&mut self, addr: u64, access_size: u8) -> hypercraft::HyperResult<u64> {
        // debug!("this is mmio read addr:{:#x}", addr);
        if self.ops.is_some() {
            let offset = addr - self.address;
            let read_func = &*self.ops.as_ref().unwrap().read;
            let ret = read_func(offset, access_size)?;
            Ok(ret)
        } else {
            let offset = addr - self.address;
            let actual_addr = self.actual_address + offset;
            // debug!(
            //     "read this is ops is none offset:{:#x} actual_addr:{:#x}",
            //     offset, actual_addr
            // );
            let data = match access_size {
                1 => unsafe { read(actual_addr as *const u8) as u64 },
                2 => unsafe { read(actual_addr as *const u16) as u64 },
                4 => unsafe { read(actual_addr as *const u32) as u64 },
                8 => unsafe { read(actual_addr as *const u64) },
                _ => return Err(HyperError::InValidMmioRead),
            };
            // debug!("read data:{:#x}", data);
            Ok(data)
        }
    }

    fn write(&mut self, addr: u64, access_size: u8, value: u64) -> hypercraft::HyperResult {
        debug!("this is mmio write addr:{:#x} value:{:#x}", addr, value);
        if self.ops.is_some() {
            let offset = addr - self.address;
            let write_func = &*self.ops.as_ref().unwrap().write;
            let value_bytes = value.to_le_bytes();
            // debug!("value_bytes: {:?}", value_bytes);
            let value_byte: &[u8] = match access_size {
                1 => &value_bytes[0..1],
                2 => &value_bytes[0..2],
                4 => &value_bytes[0..4],
                8 => &value_bytes[0..8],
                _ => return Err(HyperError::InValidMmioWrite),
            };
            write_func(offset, access_size, value_byte)
        } else {
            let offset = addr - self.address;
            let actual_addr = self.actual_address + offset;
            // debug!("write this is ops is none offset:{:#x}", offset);
            match access_size {
                1 => unsafe { write(actual_addr as *mut u8, value as u8) },
                2 => unsafe { write(actual_addr as *mut u16, value as u16) },
                4 => unsafe { write(actual_addr as *mut u32, value as u32) },
                8 => unsafe { write(actual_addr as *mut u64, value) },
                _ => return Err(HyperError::InValidMmioWrite),
            };
            Ok(())
        }
    }
}

pub struct PciBarAllocator {
    mem32_alloc: BTreeMap<u64, u64>,
    mem64_alloc: BTreeMap<u64, u64>,
    io_alloc: BTreeMap<u64, u64>,
}

impl PciBarAllocator {
    pub fn new() -> Self {
        Self {
            mem32_alloc: BTreeMap::new(),
            mem64_alloc: BTreeMap::new(),
            io_alloc: BTreeMap::new(),
        }
    }

    pub fn alloc(&mut self, region_type: RegionType, size: u64) -> HyperResult<u64> {
        let (alloc_map, base, limit) = match region_type {
            RegionType::Mem32Bit => (
                &mut self.mem32_alloc,
                PCI_EMUL_MEMBASE32,
                PCI_EMUL_MEMLIMIT32,
            ),
            RegionType::Mem64Bit => (
                &mut self.mem64_alloc,
                PCI_EMUL_MEMBASE64,
                PCI_EMUL_MEMLIMIT64,
            ),
            RegionType::Io => (&mut self.io_alloc, PCI_EMUL_IOBASE, PCI_EMUL_IOLIMIT),
        };
        debug!("alloc map:{:#?}", alloc_map);
        let mut addr = base;
        debug!("addr in alloc begin:{:#x}", addr);
        for (&start, &end) in alloc_map.iter() {
            if addr + size <= start {
                alloc_map.insert(addr, addr + size);
                debug!("this is addr in alloc map:{:#x}", addr);
                return Ok(addr);
            }
            addr = end;
        }

        if addr + size <= limit {
            alloc_map.insert(addr, addr + size);
            debug!("after return addr in alloc map:{:#x}", addr);
            return Ok(addr);
        }

        Err(HyperError::InvalidBarAddress)
    }

    pub fn alloc_addr(
        &mut self,
        region_type: RegionType,
        size: u64,
        specific_addr: u64,
    ) -> HyperResult<u64> {
        // align up to 4KB
        let size = (size + 0xfff) & !0xfff;
        let (alloc_map, base, limit) = match region_type {
            RegionType::Mem32Bit => (
                &mut self.mem32_alloc,
                PCI_EMUL_MEMBASE32,
                PCI_EMUL_MEMLIMIT32,
            ),
            RegionType::Mem64Bit => (
                &mut self.mem64_alloc,
                PCI_EMUL_MEMBASE64,
                PCI_EMUL_MEMLIMIT64,
            ),
            RegionType::Io => (&mut self.io_alloc, PCI_EMUL_IOBASE, PCI_EMUL_IOLIMIT),
        };

        if specific_addr < base || specific_addr + size > limit {
            return Err(HyperError::InvalidBarAddress);
        }

        for (&start, &end) in alloc_map.iter() {
            if specific_addr >= start && specific_addr + size <= end {
                return Err(HyperError::InvalidBarAddress);
            }
        }

        alloc_map.insert(specific_addr, specific_addr + size);
        Ok(specific_addr)
    }

    pub fn dealloc(&mut self, region_type: RegionType, addr: u64) -> HyperResult<()> {
        let alloc_map = match region_type {
            RegionType::Mem32Bit => &mut self.mem32_alloc,
            RegionType::Mem64Bit => &mut self.mem64_alloc,
            RegionType::Io => &mut self.io_alloc,
        };

        alloc_map.remove(&addr);
        Ok(())
    }
}

/// Capbility ID defined by PCIe/PCI spec.
pub enum CapId {
    Msi = 0x05,
    Pcie = 0x10,
    Msix,
}

/// Offset of registers in PCIe capability register.
enum PcieCap {
    CapReg = 0x02,
    DevCap = 0x04,
    DevCtl = 0x08,
    DevStat = 0x0a,
    LinkCap = 0x0c,
    LinkStat = 0x12,
    SlotCap = 0x14,
    SlotCtl = 0x18,
    SlotStat = 0x1a,
    RootCtl = 0x1c,
    DevCap2 = 0x24,
    DevCtl2 = 0x28,
    LinkCap2 = 0x2c,
    LinkCtl2 = 0x30,
}

/// Device/Port Type in PCIe capability register.
pub enum PcieDevType {
    PcieEp,
    LegacyPcieEp,
    RootPort = 0x04,
    UpPort,
    DownPort,
    PciePciBridge,
    PciPcieBridge,
    Rciep,
    RcEventCol,
}

/// Configuration space of PCI/PCIe device.
#[derive(Clone)]
pub struct PciConfig<B: BarAllocTrait> {
    /// Configuration space data.
    pub config: Vec<u8>,
    /// Mask of writable bits.
    pub write_mask: Vec<u8>,
    /// Mask of bits which are cleared when written.
    pub write_clear_mask: Vec<u8>,
    /// BARs.
    pub bars: Vec<Bar>,
    /// Base offset of the last PCI standard capability.
    pub last_cap_end: u16,
    /// Base offset of the last PCIe extended capability.
    pub last_ext_cap_offset: u16,
    /// End offset of the last PCIe extended capability.
    pub last_ext_cap_end: u16,
    /// MSI-X information.
    pub msix: Option<Arc<Mutex<Msix>>>,
    /// Phantom data.
    _phantom: PhantomData<B>,
}

impl<B: BarAllocTrait> PciConfig<B> {
    /// Construct new PciConfig entity.
    ///
    /// # Arguments
    ///
    /// * `config_size` - Configuration size in bytes.
    /// * `nr_bar` - Number of BARs.
    pub fn new(config_size: usize, nr_bar: u8) -> Self {
        let mut bars = Vec::new();
        for _ in 0..nr_bar as usize {
            bars.push(Bar {
                region_type: RegionType::Mem32Bit,
                address: 0,
                actual_address: 0,
                size: 0,
                ops: None,
            });
        }

        PciConfig {
            config: vec![0; config_size],
            write_mask: vec![0; config_size],
            write_clear_mask: vec![0; config_size],
            bars,
            last_cap_end: PCI_CONFIG_HEAD_END as u16,
            last_ext_cap_offset: 0,
            last_ext_cap_end: PCI_CONFIG_SPACE_SIZE as u16,
            msix: None,
            _phantom: PhantomData,
        }
    }

    /// Init write_mask for all kinds of PCI/PCIe devices, including bridges.
    pub fn init_common_write_mask(&mut self) -> Result<()> {
        self.write_mask[CACHE_LINE_SIZE as usize] = 0xff;
        self.write_mask[INTERRUPT_LINE as usize] = 0xff;
        le_write_u16(
            &mut self.write_mask,
            COMMAND as usize,
            COMMAND_IO_SPACE
                | COMMAND_MEMORY_SPACE
                | COMMAND_BUS_MASTER
                | COMMAND_INTERRUPT_DISABLE
                | COMMAND_SERR_ENABLE,
        )?;

        let mut offset = PCI_CONFIG_HEAD_END as usize;
        while offset < self.config.len() {
            le_write_u32(&mut self.write_mask, offset, 0xffff_ffff)?;
            offset += 4;
        }

        Ok(())
    }

    /// Init write_clear_mask for all kind of PCI/PCIe devices, including bridges.
    pub fn init_common_write_clear_mask(&mut self) -> Result<()> {
        le_write_u16(
            &mut self.write_clear_mask,
            STATUS as usize,
            STATUS_PARITY_ERROR
                | STATUS_SIG_TARGET_ABORT
                | STATUS_RECV_TARGET_ABORT
                | STATUS_RECV_MASTER_ABORT
                | STATUS_SIG_SYS_ERROR
                | STATUS_DETECT_PARITY_ERROR,
        )
    }

    /// Init write_mask especially for bridge devices.
    pub fn init_bridge_write_mask(&mut self) -> Result<()> {
        self.write_mask[IO_BASE as usize] = 0xf0;
        self.write_mask[IO_LIMIT as usize] = 0xf0;
        le_write_u32(&mut self.write_mask, PRIMARY_BUS_NUM as usize, 0xffff_ffff)?;
        le_write_u32(&mut self.write_mask, MEMORY_BASE as usize, 0xfff0_fff0)?;
        le_write_u32(&mut self.write_mask, PREF_MEMORY_BASE as usize, 0xfff0_fff0)?;
        le_write_u64(
            &mut self.write_mask,
            PREF_MEM_BASE_UPPER as usize,
            0xffff_ffff_ffff_ffff,
        )?;
        le_write_u16(
            &mut self.write_mask,
            BRIDGE_CONTROL as usize,
            BRIDGE_CTL_PARITY_ENABLE
                | BRIDGE_CTL_SERR_ENABLE
                | BRIDGE_CTL_ISA_ENABLE
                | BRIDGE_CTL_VGA_ENABLE
                | BRIDGE_CTL_VGA_16BIT_DEC
                | BRIDGE_CTL_SEC_BUS_RESET
                | BRIDGE_CTL_FAST_BACK
                | BRIDGE_CTL_DISCARD_TIMER
                | BRIDGE_CTL_SEC_DISCARD_TIMER
                | BRIDGE_CTL_DISCARD_TIMER_SERR_E,
        )?;
        Ok(())
    }

    /// Init write_clear_mask especially for bridge devices.
    pub fn init_bridge_write_clear_mask(&mut self) -> Result<()> {
        le_write_u16(
            &mut self.write_clear_mask,
            BRIDGE_CONTROL as usize,
            BRIDGE_CTL_DISCARD_TIMER_STATUS,
        )
    }

    /// Common reading from configuration space.
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset in the configuration space from which to read.
    /// * `data` - Buffer to put read data.
    pub fn read(&mut self, offset: usize, buf: &mut [u8]) {
        if let Err(err) = self.validate_config_boundary(offset, buf) {
            warn!("invalid read: {:?}", err);
            return;
        }

        let size = buf.len();
        debug!(
            "read offset: {} content:: {:?}",
            offset,
            &self.config[offset..(offset + size)] as &[u8]
        );

        buf[..].copy_from_slice(&self.config[offset..(offset + size)]);
    }

    fn validate_config_boundary(&self, offset: usize, data: &[u8]) -> Result<()> {
        // According to pcie specification 7.2.2.2 PCI Express Device Requirements:
        if data.len() > 4 {
            return Err(HyperError::PciError(PciError::InvalidConf(
                String::from("data size"),
                format!("{}", data.len()),
            )));
        }

        offset
            .checked_add(data.len())
            .filter(|&end| end <= self.config.len())
            .ok_or_else(|| {
                HyperError::PciError(PciError::InvalidConf(
                    String::from("config size"),
                    format!("offset {} with len {}", offset, data.len()),
                ))
            })?;

        Ok(())
    }
    /// # Arguments
    ///
    /// * `offset` - Offset in the configuration space from which to write.
    /// * `data` - Data to write.
    /// * `dev_id` - Device id to send MSI/MSI-X.
    pub fn write(&mut self, mut offset: usize, data: &[u8], dev_id: u16) {
        debug!("write offset: {} data: {:?}", offset, data);
        if let Err(err) = self.validate_config_boundary(offset, data) {
            error!("invalid write: {:?}", err);
            return;
        }

        // write data to config space.
        let cloned_data = data.to_vec();
        let old_offset = offset;
        for data in &cloned_data {
            self.config[offset] = (self.config[offset] & (!self.write_mask[offset]))
                | (data & self.write_mask[offset]);
            self.config[offset] &= !(data & self.write_clear_mask[offset]);
            offset += 1;
        }

        // Expansion ROM Base Address: 0x30-0x33 for endpoint, 0x38-0x3b for bridge.
        let (bar_num, rom_addr) = match self.config[HEADER_TYPE as usize] & HEADER_TYPE_BRIDGE {
            HEADER_TYPE_BRIDGE => (BAR_NUM_MAX_FOR_BRIDGE as usize, ROM_ADDRESS_BRIDGE),
            _ => (BAR_NUM_MAX_FOR_ENDPOINT as usize, ROM_ADDRESS_ENDPOINT),
        };

        let size = data.len();
        // SAFETY: checked in "validate_config_boundary".
        // check if command bit or bar region or expansion rom base addr changed, then update it.
        let cmd_overlap = ranges_overlap(old_offset, size, COMMAND as usize, 1).unwrap();
        if cmd_overlap
            || ranges_overlap(old_offset, size, BAR_0 as usize, REG_SIZE * bar_num).unwrap()
            || ranges_overlap(old_offset, size, rom_addr, 4).unwrap()
        {
            if let Err(e) = self.update_bar_mapping(false) {
                error!("{:?}", e);
            }
        }

        if let Some(msix) = &mut self.msix {
            msix.lock()
                .write_config(&self.config, dev_id, old_offset, data);
        }
    }

    /// Reset type1 specific configuration space.
    pub fn reset_bridge_regs(&mut self) -> Result<()> {
        le_write_u32(&mut self.config, PRIMARY_BUS_NUM as usize, 0)?;

        self.config[IO_BASE as usize] = 0xff;
        self.config[IO_LIMIT as usize] = 0;
        // set memory/pref memory's base to 0xFFFF and limit to 0.
        le_write_u32(&mut self.config, MEMORY_BASE as usize, 0xffff)?;
        le_write_u32(&mut self.config, PREF_MEMORY_BASE as usize, 0xffff)?;
        le_write_u64(&mut self.config, PREF_MEM_BASE_UPPER as usize, 0)?;
        Ok(())
    }

    fn reset_single_writable_reg(&mut self, offset: usize) -> Result<()> {
        let writable_command = le_read_u16(&self.write_mask, offset).unwrap()
            | le_read_u16(&self.write_clear_mask, offset).unwrap();
        let old_command = le_read_u16(&self.config, offset).unwrap();

        le_write_u16(&mut self.config, offset, old_command & !writable_command)
    }

    /// Reset bits that's writable in the common configuration fields for both type0 and type1
    /// devices.
    pub fn reset_common_regs(&mut self) -> Result<()> {
        self.reset_single_writable_reg(COMMAND as usize)?;
        self.reset_single_writable_reg(STATUS as usize)?;
        self.reset_single_writable_reg(INTERRUPT_LINE as usize)?;
        self.config[CACHE_LINE_SIZE as usize] = 0;

        Ok(())
    }

    /// General reset process for pci devices
    pub fn reset(&mut self) -> Result<()> {
        // if let Some(intx) = &self.intx {
        //     intx.lock().reset();
        // }

        self.reset_common_regs()?;

        if let Err(e) = self.update_bar_mapping(true) {
            error!("{:?}", e);
        }

        if let Some(msix) = &self.msix {
            msix.lock().reset();
        }

        Ok(())
    }

    /// Get base offset of the capability in PCIe/PCI configuration space.
    ///
    /// # Arguments
    ///
    /// * `cap_id` - Capability ID.
    pub fn find_pci_cap(&self, cap_id: u8) -> usize {
        let mut offset = self.config[CAP_LIST as usize];
        let mut cache_offsets = BTreeSet::new();
        cache_offsets.insert(offset);
        loop {
            let cap = self.config[offset as usize];
            if cap == cap_id {
                return offset as usize;
            }

            offset = self.config[offset as usize + NEXT_CAP_OFFSET as usize];
            if offset == 0 || cache_offsets.contains(&offset) {
                return 0xff;
            }
            cache_offsets.insert(offset);
        }
    }

    /// Get base address of BAR.
    ///
    /// # Arguments
    ///
    /// * `id` - Index of the BAR.
    pub fn get_bar_address(&self, id: usize) -> u64 {
        let command = le_read_u16(&self.config, COMMAND as usize).unwrap();
        let offset: usize = BAR_0 as usize + id * REG_SIZE;
        if self.config[offset] & BAR_IO_SPACE > 0 {
            if command & COMMAND_IO_SPACE == 0 {
                return BAR_SPACE_UNMAPPED;
            }
            let bar_val = le_read_u32(&self.config, offset).unwrap();
            return (bar_val & IO_BASE_ADDR_MASK) as u64;
        }

        if command & COMMAND_MEMORY_SPACE == 0 {
            return BAR_SPACE_UNMAPPED;
        }
        match self.bars[id].region_type {
            RegionType::Io => BAR_SPACE_UNMAPPED,
            RegionType::Mem32Bit => {
                let bar_val = le_read_u32(&self.config, offset).unwrap();
                (bar_val & MEM_BASE_ADDR_MASK as u32) as u64
            }
            RegionType::Mem64Bit => {
                let bar_val = le_read_u64(&self.config, offset).unwrap();
                bar_val & MEM_BASE_ADDR_MASK
            }
        }
    }

    /// Register a bar in PciConfig::bars.
    ///
    /// # Arguments
    ///
    /// * `id` - Index of the BAR.
    /// * `ops` - RegionOps mapped for the BAR.
    /// * `region_type` - Region type of the BAR.
    /// * `prefetchable` - Indicate whether the BAR is prefetchable or not.
    /// * `size` - Size of the BAR.
    pub fn register_bar(
        &mut self,
        id: usize,
        ops: Option<RegionOps>,
        region_type: RegionType,
        prefetchable: bool,
        size: u64,
    ) -> Result<()> {
        self.validate_bar_id(id)?;
        self.validate_bar_size(region_type, size)?;
        let offset: usize = BAR_0 as usize + id * REG_SIZE;
        let size = if region_type == RegionType::Io {
            size
        } else {
            // align up to 4KB
            (size + 0xfff) & !0xfff
        };
        match region_type {
            RegionType::Io => {
                let write_mask = !(size - 1) as u32;
                le_write_u32(&mut self.write_mask, offset, write_mask).unwrap();
                self.config[offset] = BAR_IO_SPACE;
            }
            RegionType::Mem32Bit => {
                let write_mask = !(size - 1) as u32;
                le_write_u32(&mut self.write_mask, offset, write_mask).unwrap();
            }
            RegionType::Mem64Bit => {
                let write_mask = !(size - 1);
                le_write_u64(&mut self.write_mask, offset, write_mask).unwrap();
                self.config[offset] = BAR_MEM_64BIT;
            }
        }
        if prefetchable {
            self.config[offset] |= BAR_PREFETCH;
        }
        debug!("this is register bar\n");
        // let mut allocator = PCI_BAR_ALLOCATOR.lock();
        // let addr = allocator.alloc(region_type, size)?;
        // let actual_addr = B::alloc(region_type, size)?;

        // Hard code, delete it!!!
        let addr = match region_type {
            RegionType::Io => BAR_SPACE_UNMAPPED,
            RegionType::Mem32Bit => 0xfebd5000,
            RegionType::Mem64Bit => 0xfe000000,
        };

        self.bars[id].ops = ops;
        self.bars[id].region_type = region_type;
        // self.bars[id].address = BAR_SPACE_UNMAPPED;
        self.bars[id].address = addr;
        self.bars[id].actual_address = addr;
        self.bars[id].size = size;

        // Write the front part of addr into self.config[offset + 4] to self.bars[id].actual_address = actual_addr;self.config[offset + 31]
        let length = match region_type {
            RegionType::Io => BAR_SPACE_UNMAPPED,
            RegionType::Mem32Bit => 4,
            RegionType::Mem64Bit => 8,
        } as usize;
        for i in 0..length {
            if i == 0 {
                self.config[offset + i] |= (addr as u8) & !0xf;
            } else {
                self.config[offset + i] |= (addr >> (i * 8)) as u8;
            }
        }
        let get_addr = self.get_bar_address(id);
        debug!(
            "after register content:: {:?} addr:{:#x}",
            &self.config[offset..(offset + 4)] as &[u8],
            get_addr
        );
        Ok(())
    }

    /// Unregister region in PciConfig::bars.
    ///
    /// # Arguments
    ///
    /// * `bus` - The bus which region registered.
    pub fn unregister_bars(&mut self, _bus: &Arc<Mutex<PciBus<B>>>) -> Result<()> {
        // let locked_bus = bus.lock();
        for (i, bar) in self.bars.iter_mut().enumerate() {
            if bar.address == BAR_SPACE_UNMAPPED || bar.size == 0 {
                continue;
            }
            // Invalid the bar region
            {
                let mut allocator = PCI_BAR_ALLOCATOR.lock();
                allocator.dealloc(bar.region_type, bar.address)?;
            }
            // B::dealloc(bar.region_type, bar.actual_address, bar.size)?;
            bar.address = BAR_SPACE_UNMAPPED;
            bar.actual_address = BAR_SPACE_UNMAPPED;
            bar.size = 0;
            bar.ops = None;
            for j in 0..4 {
                self.config[BAR_0 as usize + i + j] = 0;
            }
        }

        Ok(())
    }

    /// Update bar space mapping once the base address is updated by the guest.
    pub fn update_bar_mapping(&mut self, is_empty: bool) -> Result<()> {
        for id in 0..self.bars.len() {
            if self.bars[id].size == 0 {
                continue;
            }

            let new_addr: u64 = self.get_bar_address(id);
            debug!(
                "[update_bar_mapping] id: {}, addr:{:#x} new_addr: {:#x}",
                id, self.bars[id].address, new_addr
            );
            // if the bar is not updated, just skip it.
            if self.bars[id].address == new_addr {
                debug!("bar {} is not updated", id);
                continue;
            }

            // first unmmap origin bar region
            if self.bars[id].address != BAR_SPACE_UNMAPPED {
                // Invalid the bar region
                {
                    let mut allocator = PCI_BAR_ALLOCATOR.lock();
                    allocator.dealloc(self.bars[id].region_type, self.bars[id].address);
                }
                self.bars[id].address = BAR_SPACE_UNMAPPED;
            }

            if is_empty {
                return Ok(());
            }

            // map new region
            if new_addr != BAR_SPACE_UNMAPPED {
                self.bars[id].address = new_addr;
                // let mut allocator = PCI_BAR_ALLOCATOR.lock();
                // allocator.alloc_addr(self.bars[id].region_type, self.bars[id].size, new_addr)?;
                // Write the front part of addr into self.config[offset + 4] to self.config[offset + 31]
                let offset = BAR_0 as usize + id as usize * REG_SIZE;
                for i in 0..4 {
                    if i == 0 {
                        self.config[offset + i] |= (new_addr as u8) & !0xf;
                    } else {
                        self.config[offset + i] |= (new_addr >> (i * 8)) as u8;
                    }
                }
            }
        }
        Ok(())
    }

    /// Find a PIO BAR by Port.
    pub fn find_pio(&self, port: u16) -> Option<&Bar> {
        self.bars
            .iter()
            .find(|bar| bar.region_type == RegionType::Io && bar.port_range().contains(&port))
    }

    /// Find a MMIO BAR by Address.
    pub fn find_mmio(&self, addr: u64) -> Option<&Bar> {
        self.bars.iter().find(|bar| {
            bar.region_type == RegionType::Mem64Bit
                || bar.region_type == RegionType::Mem32Bit && bar.mmio_range().contains(&addr)
        })
    }

    /// Add a pci standard capability in the configuration space.
    ///
    /// # Arguments
    ///
    /// * `id` - Capability ID.
    /// * `size` - Size of the capability.
    pub fn add_pci_cap(&mut self, id: u8, size: usize) -> Result<usize> {
        // offset is the base offset of the next capability.
        let offset = self.last_cap_end as usize;
        if offset + size > PCI_CONFIG_SPACE_SIZE {
            return Err(HyperError::PciError(PciError::AddPciCap(id, size)));
        }
        // every time add the new capability to the head of the capability list.
        // [0:7]: id
        self.config[offset] = id;
        // [8:15]: next capability offset. the first saved capablilty(address by 0x34)
        self.config[offset + NEXT_CAP_OFFSET as usize] = self.config[CAP_LIST as usize];
        // update the head of the capability list.
        self.config[CAP_LIST as usize] = offset as u8;
        // [4]: Capabilities List. This optional read-only bit indicates whether or not this device implements the pointer for a New Capabilities linked list at offset 34h.
        self.config[STATUS as usize] |= STATUS_CAP_LIST as u8;

        // caculate how many regs this capability will occupy.
        let regs_num = if size % REG_SIZE == 0 {
            size / REG_SIZE
        } else {
            size / REG_SIZE + 1
        };
        // update write_mask and mov capability to next offset.
        for _ in 0..regs_num {
            le_write_u32(&mut self.write_mask, self.last_cap_end as usize, 0)?;
            self.last_cap_end += REG_SIZE as u16;
        }
        // return this capability offset.
        Ok(offset)
    }

    /// Add a pcie extended capability in the configuration space.
    ///
    /// # Arguments
    ///
    /// * `id` - Capability ID.
    /// * `size` - Size of the capability.
    /// * `version` - Capability version.
    pub fn add_pcie_ext_cap(&mut self, id: u16, size: usize, version: u32) -> Result<usize> {
        let offset = self.last_ext_cap_end as usize;
        if offset + size > PCIE_CONFIG_SPACE_SIZE {
            return Err(HyperError::PciError(PciError::AddPcieExtCap(id, size)));
        }

        let regs_num = if size % REG_SIZE == 0 {
            size / REG_SIZE
        } else {
            size / REG_SIZE + 1
        };

        for _ in 0..regs_num {
            le_write_u32(&mut self.write_mask, self.last_ext_cap_end as usize, 0)?;
            self.last_ext_cap_end += REG_SIZE as u16;
        }
        le_write_u32(
            &mut self.config,
            offset,
            id as u32 | (version << PCI_EXT_CAP_VER_SHIFT),
        )?;
        if self.last_ext_cap_offset != 0 {
            let old_value = le_read_u32(&self.config, self.last_ext_cap_offset as usize)?;
            le_write_u32(
                &mut self.config,
                self.last_ext_cap_offset as usize,
                old_value | ((offset as u32) << PCI_EXT_CAP_NEXT_SHIFT),
            )?;
        }
        self.last_ext_cap_offset = offset as u16;

        Ok(offset)
    }

    /// Calculate the next extended cap size from pci config space.
    ///
    /// # Arguments
    ///
    /// * `pos` - next extended capability offset.
    pub fn get_ext_cap_size(&self, pos: usize) -> usize {
        let mut cap_offset = PCI_CONFIG_SPACE_SIZE;
        let mut end_pos = PCIE_CONFIG_SPACE_SIZE;

        while (PCI_CONFIG_SPACE_SIZE..PCIE_CONFIG_SPACE_SIZE).contains(&cap_offset) {
            let header = le_read_u32(&self.config, cap_offset).unwrap();
            cap_offset = pci_ext_cap_next(header);
            if cap_offset > pos && cap_offset < end_pos {
                end_pos = cap_offset;
            }
        }

        end_pos - pos
    }

    fn validate_bar_id(&self, id: usize) -> Result<()> {
        if (self.config[HEADER_TYPE as usize] == HEADER_TYPE_ENDPOINT
            && id >= BAR_NUM_MAX_FOR_ENDPOINT as usize)
            || (self.config[HEADER_TYPE as usize] == HEADER_TYPE_BRIDGE
                && id >= BAR_NUM_MAX_FOR_BRIDGE as usize)
        {
            return Err(HyperError::PciError(PciError::InvalidConf(
                String::from("Bar id"),
                format!("{}", id),
            )));
        }
        Ok(())
    }

    fn validate_bar_size(&self, bar_type: RegionType, size: u64) -> Result<()> {
        if !size.is_power_of_two()
            || (bar_type == RegionType::Io && size < MINIMUM_BAR_SIZE_FOR_PIO as u64)
            || (bar_type == RegionType::Mem32Bit && size > u32::MAX as u64)
            || (bar_type == RegionType::Io && size > u16::MAX as u64)
        {
            return Err(HyperError::PciError(PciError::InvalidConf(
                String::from("Bar size of type ") + format!("{}", bar_type).as_str(),
                format!("{}", size),
            )));
        }
        Ok(())
    }

    /// check if the msix is valid
    pub fn revise_msix_vector(&self, vector_nr: u32) -> bool {
        if self.msix.is_none() {
            return false;
        }

        let table_len = self.msix.as_ref().unwrap().lock().table.len();
        let max_vector = table_len / MSIX_TABLE_ENTRY_SIZE as usize;
        vector_nr < max_vector as u32
    }
}
