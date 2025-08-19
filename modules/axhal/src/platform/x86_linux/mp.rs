use core::sync::atomic::AtomicBool;

use memory_addr::VirtAddr;

use crate::mem::{PAGE_SIZE_4K, PhysAddr, phys_to_virt, virt_to_phys};
use crate::time::{Duration, busy_wait};

const START_PAGE_IDX: u8 = 6;
const START_PAGE_PADDR: PhysAddr = pa!(START_PAGE_IDX as usize * PAGE_SIZE_4K);
const U64_PER_PAGE: usize = PAGE_SIZE_4K / 8;

core::arch::global_asm!(
    include_str!("ap_start.S"),
    start_page_paddr = const START_PAGE_PADDR.as_usize(),
);

unsafe extern "C" {
    unsafe fn ap_entry32();
    unsafe fn ap_start();
    unsafe fn ap_end();
}

static mut BACKUP_PAGE: [u64; U64_PER_PAGE] = [0; U64_PER_PAGE];

unsafe fn setup_startup_page<F>(stack_top: PhysAddr, boot_fn: F)
where
    F: FnOnce(),
{
    let start_page_ptr = phys_to_virt(START_PAGE_PADDR).as_mut_ptr() as *mut u64;
    let start_page = unsafe { core::slice::from_raw_parts_mut(start_page_ptr, U64_PER_PAGE) };

    // Since start page located at START_PAGE_PADDR belongs to Linux's physical address space.
    // We construct a backup space for start page, after `start_ap`, we just copy the backup space back.
    unsafe { BACKUP_PAGE.copy_from_slice(&start_page) };

    unsafe {
        core::ptr::copy_nonoverlapping(
            ap_start as *const u64,
            start_page_ptr,
            (ap_end as usize - ap_start as usize) / 8,
        );
    }

    // We need to use physical address here.
    // Since current physical to virtual address is not identical mapped with offset 0xffff_ff80_0000_0000.
    let ap_entry_virt = VirtAddr::from_usize(ap_entry32 as usize);
    let ap_entry_phys = virt_to_phys(ap_entry_virt);

    start_page[U64_PER_PAGE - 2] = stack_top.as_usize() as u64; // stack_top
    start_page[U64_PER_PAGE - 1] = ap_entry_phys.as_usize() as _; // entry

    boot_fn();

    // Restore the start page.
    unsafe { start_page.copy_from_slice(&BACKUP_PAGE) };
}

/// Starts the given secondary CPU with its boot stack.
/// Returns true if the caller should wait for the CPU to be ready.
pub fn start_secondary_cpu(core_id: usize, stack_top: PhysAddr) -> bool {
    // DO not boot CPUs that are reserved for host Linux.
    if super::context::core_id_is_reserved(core_id) {
        info!(
            "Core [{}] APIC id {:?} is reserved for Linux, skip",
            core_id,
            super::apic::cpu_id_to_apic_id(core_id),
        );
        return false;
    }

    // This is totally hack.
    let apic_id = super::apic::cpu_id_speculate_apic_id(core_id);

    let boot_fn = || {
        use x86::apic::x2apic::X2APIC;
        use x86::apic::{ApicControl, ApicId};

        info!("Starting secondary Core [{}] APIC_ID {}", core_id, apic_id);

        // INIT-SIPI-SIPI Sequence
        // Ref:
        // 1. Intel SDM Vol 3C, Section 10.4.4, MP Initialization Example
        // 2. Linux source code arch/x86/kernel/smpboot.c

        debug!("init with x86::apic::X2APIC");

        let mut lapic = X2APIC::new();
        let apic_id = ApicId::X2Apic(apic_id);
        unsafe {
            debug!("ipi_init");
            lapic.ipi_init(apic_id);
            busy_wait(Duration::from_micros(10)); // 10ms

            // According to <https://github.com/torvalds/linux/commit/1a744cb356c57303fc97eb15a298032170f841fa>.
            // Modern processor families do not require the 10ms delay
            // in cpu_up() to de-assert INIT. This speeds up boot
            // and resume by 10ms per (application) processor.

            // First SIPI
            lapic.ipi_startup(apic_id, START_PAGE_IDX);
            // Give the other CPU some time to accept the IPI.
            // Linux use `udelay(10);`
            busy_wait(Duration::from_millis(10)); // 10us

            // Second SIPI
            lapic.ipi_startup(apic_id, START_PAGE_IDX);
            // Give the other CPU some time to accept the IPI.
            busy_wait(Duration::from_millis(10)); // 10us
        }
    };

    unsafe { setup_startup_page(stack_top, boot_fn) };

    return true;
}

static SHUTDOWN_SECONDARY_CPUS: AtomicBool = AtomicBool::new(false);

pub fn shutdown_secondary_cpu(core_id: usize) {
    // DO not reset CPUs that are reserved for host Linux.
    if super::context::core_id_is_reserved(core_id) {
        error!(
            "Core [{}] APIC id {:?} is reserved for Linux, skip",
            core_id,
            super::apic::cpu_id_to_apic_id(core_id),
        );
        return;
    }

    match super::apic::cpu_id_to_apic_id(core_id) {
        Some(apic_id) => {
            debug!(
                "Shutting down ArceOS's secondary CPU {} APIC id {}",
                core_id, apic_id
            );
            super::apic::shutdown_ap(apic_id);
            busy_wait(Duration::from_micros(10)); // 10ms
        }
        None => {
            warn!(
                "Shutting down ArceOS's secondary CPU {} APIC id unknown",
                core_id
            );
        }
    };
}

pub fn shutdown_secondary_cpus() {
    // Only need to shutdown secondary CPUs once.
    if SHUTDOWN_SECONDARY_CPUS.load(core::sync::atomic::Ordering::SeqCst) {
        return;
    }

    SHUTDOWN_SECONDARY_CPUS.store(true, core::sync::atomic::Ordering::SeqCst);

    info!("Shutting down ArceOS's secondary CPUs...");

    for core_id in 0..axconfig::SMP {
        // DO not shutdown CPUs that are reserved for host Linux.
        if super::context::core_id_is_reserved(core_id) {
            trace!(
                "Core [{}] APIC id {:?} is reserved for Linux, skip",
                core_id,
                super::apic::cpu_id_to_apic_id(core_id),
            );
            continue;
        }

        match super::apic::cpu_id_to_apic_id(core_id) {
            Some(apic_id) => {
                debug!(
                    "Shutting down ArceOS's secondary CPU {} APIC id {}",
                    core_id, apic_id
                );
                super::apic::shutdown_ap(apic_id);
            }
            None => {
                warn!(
                    "Shutting down ArceOS's secondary CPU {} APIC id unknown",
                    core_id
                );
            }
        };
    }
}
