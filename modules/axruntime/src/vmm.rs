/// Some problems remain:
/// 1. How to return rt core back to Linux so we can "dynamically" control these rt-cores.
/// 2. How to co-schedual process from Linux within process inside arceos.
/// 3. How to handle memory mapping, currently both sides have a complete memory view of the other's address space.
use core::sync::atomic::Ordering;

fn is_init_ok() -> bool {
    super::INITED_CPUS.load(Ordering::Acquire) == (axconfig::SMP + 1)
}

unsafe extern "C" {
    unsafe fn main();
}

/// The main entry point of the ArceOS runtime for RT CPUs.
/// When booting from Linux and reserving CPU for RT tasks!
///
/// It is called from the `rust_entry_from_vmm` code in [axhal].
#[unsafe(no_mangle)]
pub extern "C" fn rust_arceos_main(cpu_id: usize) {
    info!("ARCEOS CPU {:x} started.", cpu_id);

    #[cfg(feature = "paging")]
    {
        info!("Initialize kernel page table...");
        vmm_remap_kernel_memory().expect("remap kernel memoy failed");
    }

    info!("Initialize platform devices...");
    axhal::platform_init();

    #[cfg(feature = "multitask")]
    axtask::init_scheduler();

    #[cfg(feature = "irq")]
    {
        info!("Initialize interrupt handlers...");
        super::init_interrupt();
    }

    info!("ARCEOS Primary CPU {:x} init OK.", cpu_id);
    super::INITED_CPUS.fetch_add(1, Ordering::Relaxed);

    while !is_init_ok() {
        core::hint::spin_loop();
    }

    info!("ARCEOS Primary CPU {:x} enter main ...", cpu_id);
    unsafe { main() };
    info!("ARCEOS main task exited: exit_code={}", 0);

    #[cfg(feature = "multitask")]
    axtask::exit(0);
    #[cfg(not(feature = "multitask"))]
    {
        debug!("main task exited: exit_code={}", 0);
        axhal::misc::terminate();
    }
}

/// The main entry point of the ArceOS-VMM **when booting from Linux and set self as VMM**!
///
/// It is called from the `vmm_cpu_entry` code in [axhal]. `cpu_id` is the ID of
/// the current CPU, and `dtb` is the address of the device tree blob. It
/// finally calls the application's `main` function after all initialization
/// work is done.
///
/// In multi-core environment, this function is called on the primary CPU.
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn rust_vmm_main(cpu_id: usize) {
    ax_println!("{}", super::LOGO);
    ax_println!(
        "\
        arch = {}\n\
        platform = {}\n\
        target = {}\n\
        smp = {}\n\
        build_mode = {}\n\
        log_level = {}\n\
        ",
        option_env!("AX_ARCH").unwrap_or(""),
        option_env!("AX_PLATFORM").unwrap_or(""),
        option_env!("AX_TARGET").unwrap_or(""),
        option_env!("AX_SMP").unwrap_or(""),
        option_env!("AX_MODE").unwrap_or(""),
        option_env!("AX_LOG").unwrap_or(""),
    );

    axlog::init();
    axlog::set_max_level(option_env!("AX_LOG").unwrap_or("")); // no effect if set `log-level-*` features
    info!("Logging is enabled.");
    info!("VMM Primary CPU {} started", cpu_id);

    info!("Found physcial memory regions:");
    for r in axhal::mem::memory_regions() {
        info!(
            "  [{:x?}, {:x?}) {} ({:?})",
            r.paddr,
            r.paddr + r.size,
            r.name,
            r.flags
        );
    }

    super::init_allocator();

    #[cfg(feature = "paging")]
    {
        info!("Initialize kernel page table...");
        vmm_remap_kernel_memory().expect("remap kernel memory failed");
    }

    info!("Initialize VMM platform...");
    axhal::vmm_platform_init();

    // info!("VMM Primary CPU {} init OK.", cpu_id);

    super::INITED_CPUS.fetch_add(1, Ordering::Relaxed);

    while !is_init_ok() {
        core::hint::spin_loop();
    }
}

#[cfg(feature = "paging")]
fn vmm_remap_kernel_memory() -> Result<(), axhal::paging::PagingError> {
    use axhal::host_memory_regions;
    use axhal::mem::{memory_regions, phys_to_virt};
    use axhal::paging::PageTable;
    use lazyinit::LazyInit;

    static KERNEL_PAGE_TABLE: LazyInit<PageTable> = LazyInit::new();

    if axhal::cpu::this_cpu_is_bsp() {
        info!("BSP CPU init KERNEL_PAGE_TABLE...");
        let mut kernel_page_table = PageTable::try_new()?;
        for r in memory_regions() {
            kernel_page_table.map_region(
                phys_to_virt(r.paddr),
                |_| r.paddr,
                r.size,
                r.flags.into(),
                false,
                false,
            )?;
        }

        for r in host_memory_regions() {
            kernel_page_table.map_region(
                phys_to_virt(r.paddr),
                |_| r.paddr,
                r.size,
                r.flags.into(),
                false,
                false,
            )?;
        }

        KERNEL_PAGE_TABLE.init_once(kernel_page_table);

        info!("KERNEL_PAGE_TABLE init success");
    }

    unsafe { axhal::arch::write_page_table_root(KERNEL_PAGE_TABLE.root_paddr()) };
    Ok(())
}
