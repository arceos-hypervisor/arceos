/// Starts the given secondary CPU with its all index and secondary index.
pub fn start_secondary_cpu(cpu_idx: usize, second_cpu_idx: usize) {
    #[cfg(plat_dyn)]
    start_secondary_cpu_dyn(cpu_idx, second_cpu_idx);
    #[cfg(not(plat_dyn))]
    start_secondary_cpu_static(cpu_idx, second_cpu_idx);
}

#[cfg(plat_dyn)]
fn start_secondary_cpu_dyn(cpu_idx: usize, _second_cpu_idx: usize) {
    let cpu_id = axplat_dyn::mem::cpu_idx_to_id(cpu_idx.into());
    axplat_dyn::mp::cpu_on(cpu_id);
}

#[allow(unused)]
#[repr(align(0x1000))]
#[derive(Clone, Copy)]
struct Stack([u8; axconfig::TASK_STACK_SIZE]);

impl Stack {
    const fn new() -> Self {
        Stack([0; axconfig::TASK_STACK_SIZE])
    }
}

#[cfg(not(plat_dyn))]
fn start_secondary_cpu_static(cpu_idx: usize, second_cpu_idx: usize) {
    use axconfig::TASK_STACK_SIZE;
    use memory_addr::VirtAddr;

    use crate::mem::virt_to_phys;

    static mut SECONDARY_BOOT_STACK: [Stack; axconfig::SMP - 1] = [Stack::new(); axconfig::SMP - 1];

    let base = &raw mut SECONDARY_BOOT_STACK as usize;

    let stack_top = virt_to_phys(VirtAddr::from(
        base + (second_cpu_idx + 1) * TASK_STACK_SIZE,
    ));

    crate::platform::mp::start_secondary_cpu(cpu_idx, stack_top);
}
