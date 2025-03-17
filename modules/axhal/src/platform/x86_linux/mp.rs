use crate::mem::{phys_to_virt, virt_to_phys, PhysAddr, VirtAddr, PAGE_SIZE_4K};
use crate::time::{busy_wait, Duration};
use core::sync::atomic::Ordering;

use super::header::HvHeader;
// use super::percpu::PerCpu;
use axconfig::{SMP, TASK_STACK_SIZE};

const START_PAGE_IDX: u8 = 6;
const START_PAGE_COUNT: usize = 1;
const START_PAGE_PADDR: PhysAddr = pa!(START_PAGE_IDX as usize * PAGE_SIZE_4K);

/// Starts the given secondary CPU with its boot stack.
pub fn start_secondary_cpu(_apic_id: usize, _stack_top: crate::mem::PhysAddr) {
    // No need
    // For CPU reserved for Linux, this step is completed by Linux.
    // For CPU reserved for RT-task, this step is completed by `start_arceos_cpus`.
}

pub fn continue_secondary_cpus() {
    // super::VMM_MAIN_INIT_OK.fetch_add(1, Ordering::Release);
    // super::VMM_MAIN_INIT_OK.store(1, Ordering::Release);
}

core::arch::global_asm!(
   include_str!("ap_start.S"),
   start_page_paddr = const START_PAGE_PADDR.as_usize(),
);

#[unsafe(link_section = ".bss.stack")]
static mut SECONDARY_BOOT_STACK: [[u8; TASK_STACK_SIZE]; SMP] = [[0; TASK_STACK_SIZE]; SMP];

/// Starts the given secondary CPU with its boot stack.
#[allow(clippy::uninit_assumed_init)]
pub fn start_arceos_cpus() {
    unsafe extern "C" {
        unsafe fn ap_entry32();
        unsafe fn ap_start();
        unsafe fn ap_end();
    }
    const U64_PER_PAGE: usize = PAGE_SIZE_4K / 8;

    let start_page_ptr = phys_to_virt(START_PAGE_PADDR).as_mut_ptr() as *mut u64;
    unsafe {
        let start_page =
            core::slice::from_raw_parts_mut(start_page_ptr, U64_PER_PAGE * START_PAGE_COUNT);

        // Since start page located at START_PAGE_PADDR belongs to Linux's physical address space.
        // We construct a backup space for start page, after `start_ap`, we just copy the backup space back.
        let mut backup: [u64; U64_PER_PAGE * START_PAGE_COUNT] =
            core::mem::MaybeUninit::uninit().assume_init();
        backup.copy_from_slice(start_page);
        core::ptr::copy_nonoverlapping(
            ap_start as *const u64,
            start_page_ptr,
            (ap_end as usize - ap_start as usize) / 8,
        );

        // We need to use physical address here.
        // Since current physical to virtual address is not identical mapped with offset 0xffff_ff80_0000_0000.
        let ap_entry_virt = VirtAddr::from(ap_entry32 as usize);
        let ap_entry_phys = virt_to_phys(ap_entry_virt);

        start_page[U64_PER_PAGE - 1] = ap_entry_phys.as_usize() as _; // entry

        let max_cpus = super::header::HvHeader::get().max_cpus;
        let mut arceos_cpu_num = 0;

        for apic_id in 0..max_cpus {
            // if PerCpu::cpu_is_booted(apic_id as usize) {
            //     continue;
            // }
            let stack_top = virt_to_phys(VirtAddr::from(
                SECONDARY_BOOT_STACK[arceos_cpu_num].as_ptr_range().end as usize,
            ))
            .as_usize();

            start_page[U64_PER_PAGE - 2] = stack_top as u64; // stack_top

            super::lapic::start_ap(apic_id, START_PAGE_IDX);
            // info!(
            //     "starting ArceOS {} CPU, apic id {}",
            //     arceos_cpu_num, apic_id
            // );

            arceos_cpu_num += 1;
            // wait for max 100ms
            busy_wait(Duration::from_millis(100)); // 100ms
        }
        // info!("starting {} CPUs for ArceOS ", arceos_cpu_num);

        start_page.copy_from_slice(&backup);
    }
}
