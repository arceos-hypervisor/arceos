use core::mem::MaybeUninit;

use lazyinit::LazyInit;

use x86_vcpu::LinuxContext;

use axconfig::SMP;

static mut LINUX_CTX_LIST: LazyInit<[MaybeUninit<LinuxContext>; SMP]> = LazyInit::new();
static mut CORE_ID_IS_RESERVED: [bool; SMP] = [false; SMP];

/// Set Linux context for current CPU.
pub fn set_linux_context(linux_sp: usize, core_id: usize) {
    unsafe {
        if LINUX_CTX_LIST.is_inited() {
            LINUX_CTX_LIST[core_id].write(LinuxContext::load_from(linux_sp));
        } else {
            let mut list = [const { MaybeUninit::uninit() }; SMP];
            list[core_id].write(LinuxContext::load_from(linux_sp));
            LINUX_CTX_LIST.init_once(list);
        }

        CORE_ID_IS_RESERVED[core_id] = true;
    }
}

pub fn get_linux_context_list() -> &'static [LinuxContext; SMP] {
    unsafe { &*(&raw const LINUX_CTX_LIST as *const [LinuxContext; SMP]) }
}

pub fn core_id_is_reserved(core_id: usize) -> bool {
    unsafe { CORE_ID_IS_RESERVED[core_id] }
}
