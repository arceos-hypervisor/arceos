// Copyright (c) 2020 Huawei Technologies Co.,Ltd. All rights reserved.
//
// StratoVirt is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan
// PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//         http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
// KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// NON-INFRINGEMENT, MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::max;
use core::sync::atomic::{AtomicU16, Ordering};
use spin::Mutex;

use crate::config::{CapId, RegionType, MINIMUM_BAR_SIZE_FOR_MMIO};
use crate::util::num_ops::{ranges_overlap, round_up};
use crate::{
    le_read_u16, le_read_u32, le_read_u64, le_write_u16, le_write_u32, le_write_u64, PciDevBase,
};
use crate::errors::PciResult as Result;

type GuestAddress = u64;

pub const MSIX_TABLE_ENTRY_SIZE: u16 = 16;
pub const MSIX_TABLE_SIZE_MAX: u16 = 0x7ff;
const MSIX_TABLE_VEC_CTL: u16 = 0x0c;
const MSIX_TABLE_MASK_BIT: u8 = 0x01;
pub const MSIX_TABLE_BIR: u16 = 0x07;
pub const MSIX_TABLE_OFFSET: u32 = 0xffff_fff8;
const MSIX_MSG_UPPER_ADDR: u16 = 0x04;
const MSIX_MSG_DATA: u16 = 0x08;

pub const MSIX_CAP_CONTROL: u8 = 0x02;
pub const MSIX_CAP_ENABLE: u16 = 0x8000;
pub const MSIX_CAP_FUNC_MASK: u16 = 0x4000;
pub const MSIX_CAP_SIZE: u8 = 12;
pub const MSIX_CAP_ID: u8 = 0x11;
pub const MSIX_CAP_TABLE: u8 = 0x04;
const MSIX_CAP_PBA: u8 = 0x08;

/// Basic data for msi vector.
// #[derive(Copy, Clone, Default)]
// pub struct MsiVector {
//     pub msg_addr_lo: u32,
//     pub msg_addr_hi: u32,
//     pub msg_data: u32,
//     pub masked: bool,
// }
#[derive(Copy, Clone, Default)]
pub struct MsiVector {
    pub msi_addr: u64,
    // [0:31]: data, [32:63]: vector control
    pub msi_data: u64,
}
/// MSI-X message structure.
#[derive(Copy, Clone)]
pub struct Message {
    /// Lower 32bit address of MSI-X address.
    pub address: u64,
    /// MSI-X data.
    pub data: u32,
}

/// MSI-X structure.
pub struct Msix {
    /// MSI-X table.
    pub table: Vec<u8>,
    pba: Vec<u8>,
    pub func_masked: bool,
    pub enabled: bool,
    pub msix_cap_offset: u16,
    pub dev_id: Arc<AtomicU16>,
    pub msi_irq_manager: Option<Arc<dyn MsiIrqManager>>,
}
/*
struct {
        int	enabled;
        int	table_bar;
        int	pba_bar;
        uint32_t table_offset;
        int	table_count;
        uint32_t pba_offset;
        int	pba_size;
        int	function_mask;
        struct msix_table_entry *table;	/* allocated at runtime */
        void	*pba_page;
        int	pba_page_offset;
    } msix;
*/
impl Msix {
    /// Construct a new MSI-X structure.
    ///
    /// # Arguments
    ///
    /// * `table_size` - Size in bytes of MSI-X table.
    /// * `pba_size` - Size in bytes of MSI-X PBA.
    /// * `msix_cap_offset` - Offset of MSI-X capability in configuration space.
    /// * `dev_id` - Dev_id for device.
    pub fn new(
        table_size: u32,
        pba_size: u32,
        msix_cap_offset: u16,
        dev_id: Arc<AtomicU16>,
        msi_irq_manager: Option<Arc<dyn MsiIrqManager>>,
    ) -> Self {
        let mut msix = Msix {
            table: vec![0; table_size as usize],
            pba: vec![0; pba_size as usize],
            func_masked: true,
            enabled: true,
            msix_cap_offset,
            dev_id,
            msi_irq_manager,
        };
        msix.mask_all_vectors();
        msix
    }

    pub fn reset(&mut self) {
        self.table.fill(0);
        self.pba.fill(0);
        self.func_masked = true;
        self.enabled = true;
        self.mask_all_vectors();
    }

    pub fn is_enabled(&self, config: &[u8]) -> bool {
        let offset: usize = self.msix_cap_offset as usize + MSIX_CAP_CONTROL as usize;
        let msix_ctl = le_read_u16(config, offset).unwrap();
        if msix_ctl & MSIX_CAP_ENABLE > 0 {
            return true;
        }
        false
    }

    pub fn is_func_masked(&self, config: &[u8]) -> bool {
        let offset: usize = self.msix_cap_offset as usize + MSIX_CAP_CONTROL as usize;
        let msix_ctl = le_read_u16(config, offset).unwrap();
        if msix_ctl & MSIX_CAP_FUNC_MASK > 0 {
            return true;
        }
        false
    }

    fn mask_all_vectors(&mut self) {
        let nr_vectors: usize = self.table.len() / MSIX_TABLE_ENTRY_SIZE as usize;
        for v in 0..nr_vectors {
            let offset: usize = v * MSIX_TABLE_ENTRY_SIZE as usize + MSIX_TABLE_VEC_CTL as usize;
            self.table[offset] |= MSIX_TABLE_MASK_BIT;
        }
    }

    pub fn is_vector_masked(&self, vector: u16) -> bool {
        if !self.enabled || self.func_masked {
            return true;
        }

        let offset = (vector * MSIX_TABLE_ENTRY_SIZE + MSIX_TABLE_VEC_CTL) as usize;
        if self.table[offset] & MSIX_TABLE_MASK_BIT == 0 {
            return false;
        }
        true
    }

    fn is_vector_pending(&self, vector: u16) -> bool {
        let offset: usize = vector as usize / 64;
        let pending_bit: u64 = 1 << (vector as u64 % 64);
        let value = le_read_u64(&self.pba, offset).unwrap();
        if value & pending_bit > 0 {
            return true;
        }
        false
    }

    fn set_pending_vector(&mut self, vector: u16) {
        let offset: usize = vector as usize / 64;
        let pending_bit: u64 = 1 << (vector as u64 % 64);
        let old_val = le_read_u64(&self.pba, offset).unwrap();
        le_write_u64(&mut self.pba, offset, old_val | pending_bit).unwrap();
    }

    fn clear_pending_vector(&mut self, vector: u16) {
        let offset: usize = vector as usize / 64;
        let pending_bit: u64 = !(1 << (vector as u64 % 64));
        let old_val = le_read_u64(&self.pba, offset).unwrap();
        le_write_u64(&mut self.pba, offset, old_val & pending_bit).unwrap();
    }

    pub fn clear_pending_vectors(&mut self) {
        let max_vector_nr = self.table.len() as u16 / MSIX_TABLE_ENTRY_SIZE;
        for v in 0..max_vector_nr {
            self.clear_pending_vector(v);
        }
    }

    fn update_irq_routing(&mut self, vector: u16, is_masked: bool) -> Result<()> {
        let entry = self.get_message(vector);
        let route = if let Some(route) = self.gsi_msi_routes.get_mut(&vector) {
            route
        } else {
            return Ok(());
        };

        let msix_vector = self.get_msix_vector(vector);

        let irq_manager = self.msi_irq_manager.as_ref().unwrap();

        if is_masked {
            irq_manager.unregister_irqfd(route.irq_fd.clone(), route.gsi as u32)?;
        } else {
            let msg = &route.msg;
            if msg.data != entry.data
                || msg.address_lo != entry.address_lo
                || msg.address_hi != entry.address_hi
            {
                irq_manager.update_route_table(route.gsi as u32, msix_vector)?;
                route.msg = entry;
            }

            irq_manager.register_irqfd(route.irq_fd.clone(), route.gsi as u32)?;
        }
        Ok(())
    } 

    pub fn get_msix_vector(&self, vector: u16) -> MsiVector {
        let entry_offset: u16 = vector * MSIX_TABLE_ENTRY_SIZE;
        let mut offset = entry_offset as usize;
        let address = le_read_u64(&self.table, offset).unwrap();
        offset = (entry_offset + MSIX_MSG_DATA) as usize;
        let data = le_read_u64(&self.table, offset).unwrap();

        MsiVector {
            msi_addr: address,
            msi_data: data,
        }
    }

    pub fn send_msix(&self, vector: u16, dev_id: u16) {
        let msix_vector = self.get_msix_vector(vector);

        let irq_manager = self.msi_irq_manager.as_ref().unwrap();
        if let Err(e) = irq_manager.trigger(msix_vector, dev_id as u32) {
            error!("Send msix error: {:?}", e);
        };
    }

    pub fn notify(&mut self, vector: u16, dev_id: u16) {
        if vector >= self.table.len() as u16 / MSIX_TABLE_ENTRY_SIZE {
            warn!("Invalid msix vector {}.", vector);
            return;
        }

        if self.is_vector_masked(vector) {
            self.set_pending_vector(vector);
            return;
        }

        self.send_msix(vector, dev_id);
    }

    pub fn write_config(&mut self, config: &[u8], dev_id: u16, offset: usize, data: &[u8]) {
        let len = data.len();
        let msix_cap_control_off: usize = self.msix_cap_offset as usize + MSIX_CAP_CONTROL as usize;
        // Only care about the bits Masked(14) & Enabled(15) in msix control register.
        // SAFETY: msix_cap_control_off is less than u16::MAX.
        // Offset and len have been checked in call function PciConfig::write.
        if !ranges_overlap(offset, len, msix_cap_control_off + 1, 1).unwrap() {
            return;
        }

        let masked: bool = self.is_func_masked(config);
        let enabled: bool = self.is_enabled(config);

        let mask_state_changed = !((self.func_masked == masked) && (self.enabled == enabled));

        self.func_masked = masked;
        self.enabled = enabled;

        if mask_state_changed && (self.enabled && !self.func_masked) {
            let max_vectors_nr: u16 = self.table.len() as u16 / MSIX_TABLE_ENTRY_SIZE;
            for v in 0..max_vectors_nr {
                if !self.is_vector_masked(v) && self.is_vector_pending(v) {
                    self.clear_pending_vector(v);
                    self.send_msix(v, dev_id);
                }
            }
        }
    }

    fn register_memory_region(
        _msix: Arc<Mutex<Self>>,
        region: &Region,
        _dev_id: Arc<AtomicU16>,
        _table_offset: u64,
        _pba_offset: u64,
    ) -> Result<()> {
        let locked_msix = msix.lock();
        let table_size = locked_msix.table.len() as u64;
        let pba_size = locked_msix.pba.len() as u64;

        let cloned_msix = msix.clone();
        let table_read = move |data: &mut [u8], _addr: GuestAddress, offset: u64| -> bool {
            if offset as usize + data.len() > cloned_msix.lock().table.len() {
                error!(
                    "It's forbidden to read out of the msix table(size: {}), with offset of {} and size of {}",
                    cloned_msix.lock().table.len(),
                    offset,
                    data.len()
                );
                return false;
            }
            let offset = offset as usize;
            data.copy_from_slice(&cloned_msix.lock().table[offset..(offset + data.len())]);
            true
        };
        let cloned_msix = msix.clone();
        let table_write = move |data: &[u8], _addr: GuestAddress, offset: u64| -> bool {
            if offset as usize + data.len() > cloned_msix.lock().table.len() {
                error!(
                    "It's forbidden to write out of the msix table(size: {}), with offset of {} and size of {}",
                    cloned_msix.lock().table.len(),
                    offset,
                    data.len()
                );
                return false;
            }
            let mut locked_msix = cloned_msix.lock();
            let vector: u16 = offset as u16 / MSIX_TABLE_ENTRY_SIZE;
            let was_masked: bool = locked_msix.is_vector_masked(vector);
            let offset = offset as usize;
            locked_msix.table[offset..(offset + 4)].copy_from_slice(data);

            let is_masked: bool = locked_msix.is_vector_masked(vector);
            if was_masked != is_masked && locked_msix.update_irq_routing(vector, is_masked).is_err()
            {
                return false;
            }

            // Clear the pending vector just when it is pending. Otherwise, it
            // will cause unknown error.
            if was_masked && !is_masked && locked_msix.is_vector_pending(vector) {
                locked_msix.clear_pending_vector(vector);
                locked_msix.notify(vector, dev_id.load(Ordering::Acquire));
            }

            true
        };
        let table_region_ops = RegionOps {
            read: Arc::new(table_read),
            write: Arc::new(table_write),
        };
        let table_region = Region::init_io_region(table_size, table_region_ops, "MsixTable");
        region
            .add_subregion(table_region, table_offset)
            .with_context(|| "Failed to register MSI-X table region.")?;

        let cloned_msix = msix.clone();
        let pba_read = move |data: &mut [u8], _addr: GuestAddress, offset: u64| -> bool {
            if offset as usize + data.len() > cloned_msix.lock().pba.len() {
                error!(
                    "Fail to read msi pba, illegal data length {}, offset {}",
                    data.len(),
                    offset
                );
                return false;
            }
            let offset = offset as usize;
            data.copy_from_slice(&cloned_msix.lock().pba[offset..(offset + data.len())]);
            true
        };
        let pba_write = move |_data: &[u8], _addr: GuestAddress, _offset: u64| -> bool { true };
        let pba_region_ops = RegionOps {
            read: Arc::new(pba_read),
            write: Arc::new(pba_write),
        };
        let pba_region = Region::init_io_region(pba_size, pba_region_ops, "MsixPba");
        region
            .add_subregion(pba_region, pba_offset)
            .with_context(|| "Failed to register MSI-X PBA region.")?;

        Ok(())
    }
}

/// MSI-X initialization.
///
/// # Arguments
///
/// * `pcidev_base ` - The Base of PCI device
/// * `bar_id` - BAR id.
/// * `vector_nr` - The number of vector.
/// * `dev_id` - Dev id.
/// * `parent_region` - Parent region which the MSI-X region registered. If none, registered in BAR.
/// * `offset_opt` - Offset of table(table_offset) and Offset of pba(pba_offset). Set the
///   table_offset and pba_offset together.
pub fn init_msix(
    pcidev_base: &mut PciDevBase,
    bar_id: usize,
    vector_nr: u32,
    dev_id: Arc<AtomicU16>,
    // parent_region: Option<&Region>,
    offset_opt: Option<(u32, u32)>,
    msi_irq_manager: Option<Arc<dyn MsiIrqManager>>,
) -> Result<()> {
    let config = &mut pcidev_base.config;
    let parent_bus = &pcidev_base.parent_bus;
    if vector_nr == 0 || vector_nr > MSIX_TABLE_SIZE_MAX as u32 + 1 {
        error!(
            "invalid msix vectors, which should be in [1, {}]",
            MSIX_TABLE_SIZE_MAX + 1
        );
    }

    let msix_cap_offset: usize = config.add_pci_cap(CapId::Msix as u8, MSIX_CAP_SIZE as usize)?;
    let mut offset: usize = msix_cap_offset + MSIX_CAP_CONTROL as usize;
    le_write_u16(&mut config.config, offset, vector_nr as u16 - 1)?;
    le_write_u16(
        &mut config.write_mask,
        offset,
        MSIX_CAP_FUNC_MASK | MSIX_CAP_ENABLE,
    )?;
    offset = msix_cap_offset + MSIX_CAP_TABLE as usize;
    let table_size = vector_nr * MSIX_TABLE_ENTRY_SIZE as u32;
    let pba_size = ((round_up(vector_nr as u64, 64).unwrap() / 64) * 8) as u32;
    let (table_offset, pba_offset) = offset_opt.unwrap_or((0, table_size));
    if ranges_overlap(
        table_offset as usize,
        table_size as usize,
        pba_offset as usize,
        pba_size as usize,
    )
    .unwrap()
    {
        error!("msix table and pba table overlapped.");
    }
    le_write_u32(&mut config.config, offset, table_offset | bar_id as u32)?;
    offset = msix_cap_offset + MSIX_CAP_PBA as usize;
    le_write_u32(&mut config.config, offset, pba_offset | bar_id as u32)?;

    // let msi_irq_manager = if let Some(pci_bus) = parent_bus.upgrade() {
    //     let locked_pci_bus = pci_bus.lock();
    //     locked_pci_bus.get_msi_irq_manager()
    // } else {
    //     error!("Msi irq controller is none");
    //     None
    // };

    let msix = Arc::new(Mutex::new(Msix::new(
        table_size,
        pba_size,
        msix_cap_offset as u16,
        dev_id.clone(),
        msi_irq_manager,
    )));
    if let Some(region) = parent_region {
        Msix::register_memory_region(
            msix.clone(),
            region,
            dev_id,
            table_offset as u64,
            pba_offset as u64,
        )?;
    } else {
        let mut bar_size = ((table_size + pba_size) as u64).next_power_of_two();
        bar_size = max(bar_size, MINIMUM_BAR_SIZE_FOR_MMIO as u64);
        let region = Region::init_container_region(bar_size, "Msix_region");
        Msix::register_memory_region(
            msix.clone(),
            &region,
            dev_id,
            table_offset as u64,
            pba_offset as u64,
        )?;
        config.register_bar(bar_id, region, RegionType::Mem32Bit, false, bar_size)?;
    }

    config.msix = Some(msix.clone());

    Ok(())
}

pub trait MsiIrqManager: Send + Sync {
    fn allocate_irq(&self, _vector: MsiVector) -> Result<u32> {
        Ok(0)
    }

    fn release_irq(&self, _irq: u32) -> Result<()> {
        Ok(())
    }

    fn register_irqfd(&self, _irq_fd: Arc<EventFd>, _irq: u32) -> Result<()> {
        Ok(())
    }

    fn unregister_irqfd(&self, _irq_fd: Arc<EventFd>, _irq: u32) -> Result<()> {
        Ok(())
    }

    fn trigger(
        &self,
        _vector: MsiVector,
        _dev_id: u32,
    ) -> Result<()> {
        Ok(())
    }
// /**
//  *@pre Pointer vm shall point to Service VM
//  */
// static void inject_msi_for_lapic_pt(struct acrn_vm *vm, uint64_t addr, uint64_t data)
// {
// 	union apic_icr icr;
// 	struct acrn_vcpu *vcpu;
// 	union msi_addr_reg vmsi_addr;
// 	union msi_data_reg vmsi_data;
// 	uint64_t vdmask = 0UL;
// 	uint32_t vdest, dest = 0U;
// 	uint16_t vcpu_id;
// 	bool phys;

// 	vmsi_addr.full = addr;
// 	vmsi_data.full = (uint32_t)data;

// 	dev_dbg(DBG_LEVEL_LAPICPT, "%s: msi_addr 0x%016lx, msi_data 0x%016lx",
// 		__func__, addr, data);

// 	if (vmsi_addr.bits.addr_base == MSI_ADDR_BASE) {
// 		vdest = vmsi_addr.bits.dest_field;
// 		phys = (vmsi_addr.bits.dest_mode == MSI_ADDR_DESTMODE_PHYS);
// 		/*
// 		 * calculate all reachable destination vcpu.
// 		 * the delivery mode of vmsi will be forwarded to ICR delievry field
// 		 * and handled by hardware.
// 		 */
// 		vdmask = vlapic_calc_dest_noshort(vm, false, vdest, phys, false);
// 		dev_dbg(DBG_LEVEL_LAPICPT, "%s: vcpu destination mask 0x%016lx", __func__, vdmask);

// 		vcpu_id = ffs64(vdmask);
// 		while (vcpu_id != INVALID_BIT_INDEX) {
// 			bitmap_clear_nolock(vcpu_id, &vdmask);
// 			vcpu = vcpu_from_vid(vm, vcpu_id);
// 			dest |= per_cpu(lapic_ldr, pcpuid_from_vcpu(vcpu));
// 			vcpu_id = ffs64(vdmask);
// 		}

// 		icr.value = 0UL;
// 		icr.bits.dest_field = dest;
// 		icr.bits.vector = vmsi_data.bits.vector;
// 		icr.bits.delivery_mode = vmsi_data.bits.delivery_mode;
// 		icr.bits.destination_mode = MSI_ADDR_DESTMODE_LOGICAL;

// 		msr_write(MSR_IA32_EXT_APIC_ICR, icr.value);
// 		dev_dbg(DBG_LEVEL_LAPICPT, "%s: icr.value 0x%016lx", __func__, icr.value);
// 	}
// }

    fn update_route_table(&self, _gsi: u32, _vector: MsiVector) -> Result<()> {
        Ok(())
    }
}