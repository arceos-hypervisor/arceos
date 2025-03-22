use core::mem::MaybeUninit;

use lazyinit::LazyInit;

use x86_vcpu::LinuxContext;

use axconfig::SMP;

static mut LINUX_CTX_LIST: LazyInit<[MaybeUninit<LinuxContext>; SMP]> = LazyInit::new();

/// Set Linux context for current CPU.
pub fn set_linux_context(linux_sp: usize, cpu_id: usize) {
    unsafe {
        if LINUX_CTX_LIST.is_inited() {
            LINUX_CTX_LIST[cpu_id].write(LinuxContext::load_from(linux_sp));
        } else {
            let mut list = [const { MaybeUninit::uninit() }; SMP];
            list[cpu_id].write(LinuxContext::load_from(linux_sp));
            LINUX_CTX_LIST.init_once(list);
        }
    }
}

pub fn get_linux_context_list() -> &'static [LinuxContext; SMP] {
    // LINUX_CTX_LIST.as_ref()
    unsafe { &*(&raw const LINUX_CTX_LIST as *const [LinuxContext; SMP]) }
}
