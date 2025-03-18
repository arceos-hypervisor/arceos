use core::mem::MaybeUninit;

use lazyinit::LazyInit;

use x86_vcpu::LinuxContext;

use axconfig::SMP;

use crate::cpu::this_cpu_id;

#[percpu::def_percpu]
static LINUX_CTX: LazyInit<LinuxContext> = LazyInit::new();

static mut LINUX_CTX_LIST: LazyInit<[MaybeUninit<LinuxContext>; SMP]> = LazyInit::new();

/// Set Linux context for current CPU.
pub fn set_linux_context(linux_sp: usize) {
    let linux_ctx = unsafe { LINUX_CTX.current_ref_mut_raw() };
    linux_ctx.init_once(LinuxContext::load_from(linux_sp));

    unsafe {
        if LINUX_CTX_LIST.is_inited() {
            LINUX_CTX_LIST[this_cpu_id()].write(LinuxContext::load_from(linux_sp));
        } else {
            let mut list = [const { MaybeUninit::uninit() }; SMP];
            list[this_cpu_id()].write(LinuxContext::load_from(linux_sp));
            LINUX_CTX_LIST.init_once(list);
        }
    }
}

/// Get Linux context for current CPU.
#[allow(unused)]
pub fn get_linux_context() -> &'static LinuxContext {
    unsafe { LINUX_CTX.current_ref_raw() }
}

/// Get Linux context for the given CPU ID.
pub fn get_linux_context_by_cpu_id(cpu_id: usize) -> &'static LinuxContext {
    unsafe { LINUX_CTX.remote_ref_raw(cpu_id) }
}

pub fn get_linux_context_list() -> &'static [LinuxContext; SMP] {
    // LINUX_CTX_LIST.as_ref()
    unsafe { &*(&raw const LINUX_CTX_LIST as *const [LinuxContext; SMP]) }
}
