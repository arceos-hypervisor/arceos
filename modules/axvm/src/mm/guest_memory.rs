use core::char::DecodeUtf16Error;

use crate::{GuestPageTable, GuestPhysAddr, GuestVirtAddr, HostPhysAddr, HostVirtAddr};
use alloc::vec::Vec;
use axhal::hv::HyperCraftHalImpl;
use axhal::mem::{phys_to_virt, PhysAddr};
use hypercraft::{
    GuestPageTableTrait, GuestPageWalkInfo, HyperError, HyperResult, VCpu, VmCpus, VM,
};
use x86_64::registers::debug;

const PAGE_FAULT_ID_FLAG: u32 = 0x00000010;
const PAGE_FAULT_P_FLAG: u32 = 0x00000001;
const PAGE_ENTRY_CNT: usize = 512;
const PAGE_SIZE: usize = 0x1000;

pub fn get_gva_content_bytes(
    guest_rip: usize,
    length: u32,
    vcpu: VCpu<HyperCraftHalImpl>,
    ept: GuestPageTable,
) -> HyperResult<Vec<u8>> {
    debug!(
        "get_gva_content_bytes: guest_rip: {:#x}, length: {:#x}",
        guest_rip, length
    );
    let gva = vcpu.gla2gva(guest_rip);
    debug!("get_gva_content_bytes: gva: {:#x}", gva);
    let gpa = gva2gpa(vcpu, ept.clone(), gva)?;
    debug!("get_gva_content_bytes: gpa: {:#x}", gpa);
    let hva = gpa2hva(ept.clone(), gpa)?;
    debug!("get_gva_content_bytes: hva: {:#x}", hva);
    let mut content = Vec::with_capacity(length as usize);
    let code_ptr = hva as *const u8;
    unsafe {
        for i in 0..length {
            let value_ptr = code_ptr.offset(i as isize);
            content.push(value_ptr.read());
        }
    }
    debug!("get_gva_content_bytes: content: {:#?}", content);
    Ok(content)
}

fn gpa2hva(ept: GuestPageTable, gpa: GuestPhysAddr) -> HyperResult<HostVirtAddr> {
    let hpa = gpa2hpa(ept, gpa)?;
    let hva = phys_to_virt(PhysAddr::from(hpa));
    Ok(usize::from(hva) as HostVirtAddr)
}

fn gpa2hpa(ept: GuestPageTable, gpa: GuestPhysAddr) -> HyperResult<HostPhysAddr> {
    ept.translate(gpa)
}

fn gva2gpa(
    vcpu: VCpu<HyperCraftHalImpl>,
    ept: GuestPageTable,
    gva: GuestVirtAddr,
) -> HyperResult<GuestPhysAddr> {
    let guest_ptw_info = vcpu.get_ptw_info();
    page_table_walk(ept, guest_ptw_info, gva)
}

// suppose it is 4-level page table
fn page_table_walk(
    ept: GuestPageTable,
    pw_info: GuestPageWalkInfo,
    gva: GuestVirtAddr,
) -> HyperResult<GuestPhysAddr> {
    debug!("page_table_walk: gva: {:#x} pw_info:{:?}", gva, pw_info);
    if pw_info.level <= 1 {
        return Ok(gva as GuestPhysAddr);
    }
    let mut addr = pw_info.top_entry;
    let mut current_level = pw_info.level;
    let mut shift = 0;
    while current_level != 0 {
        current_level -= 1;
        // get page table base addr
        addr = addr & !(PAGE_ENTRY_CNT - 1);
        let base = gpa2hva(ept.clone(), addr)?;
        shift = (current_level * pw_info.width as usize) + 12;
        let index = (gva >> shift) & (PAGE_ENTRY_CNT - 1);
        // get page table entry pointer
        let entry_ptr = unsafe { (base as *const usize).offset(index as isize) };
        // next page table addr (gpa)
        addr = unsafe { *entry_ptr };
    }

    let mut entry = addr;
    debug!("1 page_table_walk: entry: {:#x} shift:{:#x}", entry, shift);
    // ?????
    entry >>= shift;
    debug!("2 page_table_walk: entry: {:#x} shift:{:#x}", entry, shift);
    /* shift left 12bit more and back to clear XD/Prot Key/Ignored bits */
    entry <<= shift + 12;
    debug!("3 page_table_walk: entry: {:#x} shift:{:#x}", entry, shift);
    entry >>= 12;
    debug!("4 page_table_walk: entry: {:#x} shift:{:#x}", entry, shift);
    Ok((entry | (gva & (PAGE_SIZE - 1))) as GuestPhysAddr)
}
