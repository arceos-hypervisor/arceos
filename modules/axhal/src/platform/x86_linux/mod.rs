mod apic;
// It's a simplied version of LocalApic, just use for sendsipi.
mod lapic;

mod boot;
mod dtables;
mod entry;
mod uart16550;

pub mod mem;
pub mod misc;
pub mod time;

// mods for vmm usage.
mod percpu;

mod config;
mod consts;
mod header;

// #[cfg(feature = "smp")]
pub mod mp;

#[cfg(feature = "irq")]
pub mod irq {
    pub use super::apic::*;
}

pub mod console {
    pub use super::uart16550::*;
}

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use axlog::ax_println as println;

use config::HvSystemConfig;
// use error::HvResult;
use header::HvHeader;
use percpu::PerCpu;

static VMM_PRIMARY_INIT_OK: AtomicU32 = AtomicU32::new(0);
static ERROR_NUM: AtomicI32 = AtomicI32::new(0);

fn has_err() -> bool {
    ERROR_NUM.load(Ordering::Acquire) != 0
}

fn wait_for(condition: impl Fn() -> bool) {
    while !has_err() && condition() {
        core::hint::spin_loop();
    }
    if has_err() {
        println!("[Error] Other cpu init failed!")
    }
}

unsafe extern "C" {
    unsafe fn rust_vmm_main(cpu_id: usize);
    unsafe fn rust_arceos_main(cpu_id: usize) -> !;
}

fn current_cpu_id() -> usize {
    match raw_cpuid::CpuId::new().get_feature_info() {
        Some(finfo) => finfo.initial_local_apic_id() as usize,
        None => 0,
    }
}

fn vmm_primary_init_early(cpu_id: usize) {
    // We do not clear bss here.
    // Because currently the image was loaded by Linux.
    // crate::mem::clear_bss();

    crate::cpu::init_primary(cpu_id);
    self::time::init_early();

    // println!("HvHeader\n{:#?}", HvHeader::get());

    let system_config = HvSystemConfig::get();

    println!(
        "\n\
        Initializing ARCEOS on Core [{}]...\n\
        config_signature = {:?}\n\
        config_revision = {}\n\
        ",
        cpu_id,
        core::str::from_utf8(&system_config.signature),
        system_config.revision,
    );

    VMM_PRIMARY_INIT_OK.store(1, Ordering::Release);

    unsafe {
        rust_vmm_main(cpu_id);
    }
}

extern "sysv64" fn vmm_cpu_entry(cpu_data: &mut PerCpu, _linux_sp: usize) -> i32 {
    // Currently we set core 0 as Linux.
    let is_primary = cpu_data.id == 0;

    let vm_cpus = HvHeader::get().reserved_cpus();

    wait_for(|| PerCpu::entered_cpus() < vm_cpus);

    // println!(
    //     "{} CPU {} entered.",
    //     if is_primary { "Primary" } else { "Secondary" },
    //     cpu_data.id
    // );

    // First, we init primary core for VMM.
    if is_primary {
        vmm_primary_init_early(cpu_data.id as usize);
    } else {
        // wait_for_counter(&VMM_PRIMARY_INIT_OK, 1);

        // wait_for_counter(&VMM_MAIN_INIT_OK, 1);

        // vmm_secondary_init_early(cpu_data.id as usize);
    }

    let code = 0;
    // println!(
    //     "{} CPU {} return back to driver with code {}.",
    //     if is_primary { "Primary" } else { "Secondary" },
    //     cpu_data.id,
    //     code
    // );
    code
}

unsafe extern "C" fn rust_entry(_magic: usize, _mbi: usize) {
    // TODO: handle multiboot info
    // if magic == self::boot::MULTIBOOT_BOOTLOADER_MAGIC {
    //     crate::mem::clear_bss();
    //     crate::cpu::init_primary(current_cpu_id());
    //     self::uart16550::init();
    //     self::dtables::init_primary();
    //     self::time::init_early();
    //     rust_main(current_cpu_id(), 0);
    // }
}

#[allow(unused_variables)]
unsafe extern "C" fn rust_entry_from_vmm(magic: usize) {
    let cpu_id = current_cpu_id();
    info!("ARCEOS CPU entered on Core {}.", cpu_id);

    if magic == self::boot::MULTIBOOT_BOOTLOADER_MAGIC {
        crate::cpu::init_secondary(cpu_id);
        self::dtables::init_primary();
        self::time::init_early();
        rust_arceos_main(cpu_id);
    } else {
        panic!("Something is wrong during booting RT cores...");
    }
}

/// Initializes the platform devices for the primary CPU.
/// Boot arceos cpus through sendsipi.
pub fn vmm_platform_init() {
    self::lapic::init();
    self::mp::start_arceos_cpus();
}

/// Initializes the platform devices for the primary CPU.
pub fn platform_init() {
    self::apic::init_primary();
    self::time::init_primary();
}

/// Initializes the platform devices for secondary CPUs.
#[cfg(feature = "smp")]
pub fn platform_init_secondary() {
    self::apic::init_secondary();
    self::time::init_secondary();
}
