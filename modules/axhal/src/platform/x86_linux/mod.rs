mod apic;

mod boot;
mod dtables;
mod entry;
mod uart16550;

pub mod mem;
pub mod misc;
pub mod time;

// mods for vmm usage.
// mod percpu;

pub mod config;
mod consts;
pub mod context;
pub mod header;

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

static VMM_PRIMARY_INIT_OK: AtomicU32 = AtomicU32::new(0);
static ERROR_NUM: AtomicI32 = AtomicI32::new(0);

fn has_err() -> bool {
    ERROR_NUM.load(Ordering::Acquire) != 0
}

fn wait_while(condition: impl Fn() -> bool) {
    while !has_err() && condition() {
        core::hint::spin_loop();
    }
    if has_err() {
        println!("[Error] Other cpu init failed!")
    }
}

unsafe extern "C" {
    fn rust_main(cpu_id: usize, dtb: usize) -> !;
    #[cfg(feature = "smp")]
    fn rust_main_secondary(cpu_id: usize) -> !;
}

fn current_cpu_id() -> usize {
    match raw_cpuid::CpuId::new().get_feature_info() {
        Some(finfo) => finfo.initial_local_apic_id() as usize,
        None => 0,
    }
}

fn vmm_primary_init_early(cpu_id: usize) {
    println!("Primary CPU {} init early", cpu_id);
    // We do not clear bss here.
    // Because currently the image was loaded by Linux.
    // crate::mem::clear_bss();
    crate::cpu::init_primary(cpu_id);
    self::uart16550::init();
}

fn vmm_secondary_init_early(cpu_id: usize) {
    #[cfg(feature = "smp")]
    {
        println!("Secondary CPU {} init early.", cpu_id);
        crate::cpu::init_secondary(cpu_id);
    }
}

fn vmm_primary_init(cpu_id: usize) {
    self::dtables::init_primary();
    self::time::init_early();

    println!("HvHeader\n{:#?}", HvHeader::get());

    let system_config = HvSystemConfig::get();

    system_config.check();

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
}

fn vmm_secondary_init(_cpu_id: usize) {
    #[cfg(feature = "smp")]
    {
        self::dtables::init_secondary();
    }
}

/// Cores entered from Linux will call this function.
/// Cores reserved for ArceOS for other purposed will be shutdown by Linux
/// before entry and restarted through SIPI by ArceOS's `start_secondary_cpu` in mp.rs.
extern "sysv64" fn vmm_cpu_entry(core_id: usize, linux_sp: usize) -> i32 {
    let cpu_id = current_cpu_id();

    // Use Cpu ID 0 as primary core.
    // TODO: on some platform Local Apic ID may not start from Zero.
    let is_primary = core_id == 0;

    let vm_cpus = HvHeader::get().reserved_cpus();

    println!(
        "{} Core {} (LAPIC_ID {}) entered. [{}/{}]",
        if is_primary { "Primary" } else { "Secondary" },
        core_id,
        cpu_id,
        core_id,
        vm_cpus
    );

    wait_while(|| entry::entered_cpus() < vm_cpus);

    println!(
        "{} Core {} CPU {} start to initialize.",
        if is_primary { "Primary" } else { "Secondary" },
        core_id,
        cpu_id,
    );

    // First, we init primary core for VMM.
    if is_primary {
        vmm_primary_init_early(cpu_id);
    } else {
        wait_while(|| VMM_PRIMARY_INIT_OK.load(Ordering::Acquire) == 0);
        vmm_secondary_init_early(cpu_id);
    }

    // Note: this has to be done after `cpu::init_primary`.
    // Because LinuxContext will be stored in percpu area.
    context::set_linux_context(linux_sp, cpu_id);

    if is_primary {
        vmm_primary_init(cpu_id);
    } else {
        vmm_secondary_init(cpu_id);
    }

    unsafe {
        if is_primary {
            rust_main(cpu_id, 0);
        } else {
            rust_main_secondary(cpu_id);
        }
    }
}

/// Core reserved for ArceOS will entered through this function.
unsafe extern "C" fn rust_entry_secondary(magic: usize) {
    // #[cfg(feature = "smp")]
    if magic == self::boot::MULTIBOOT_BOOTLOADER_MAGIC {
        // Note: DO not call log related functions before percpu area is initialized.
        crate::cpu::init_secondary(current_cpu_id());
        self::dtables::init_secondary();
        unsafe {
            rust_main_secondary(current_cpu_id());
        }
    }
}

/// Initializes the platform devices for the primary CPU.
pub fn platform_init() {
    // Consruct LAPIC but DO NOT operate the LAPIC.
    // because the LAPIC belongs to Linux, we should not touch it.
    self::apic::init_primary(false);
    // self::time::init_primary();

    VMM_PRIMARY_INIT_OK.store(1, Ordering::Release);
    // Secondary CPUs continue to initialize.
}

/// Initializes the platform devices for secondary CPUs.
#[cfg(feature = "smp")]
pub fn platform_init_secondary() {
    self::apic::init_secondary(false);
    // self::time::init_secondary();
}
