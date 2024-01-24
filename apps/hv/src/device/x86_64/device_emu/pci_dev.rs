use spin::RwLock;
use alloc::collections::BTreeMap;
use core::fmt::{Debug, Formatter, Result};
use core::mem;
use core::slice;
use vm_config::pci_dev::PCIDevice;


pub static PCI_DEVICES: RwLock<BTreeMap<u32, PCIDevice>> = RwLock::new(BTreeMap::new());

// according to linux arch/x86/pci/early.c
pub const BUS_MASK: u32 = 0x00ff_0000;
pub const SLOT_MASK: u32 = 0x0000_f800;
pub const FUNC_MASK: u32 = 0x0000_0700;
pub const OFFSET_MASK: u16 = 0x00fc;
