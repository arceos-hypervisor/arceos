use core::sync::atomic::{AtomicU32, Ordering};

use super::header::HvHeader;
use axconfig::{SMP, TASK_STACK_SIZE};

static ENTERED_CPUS: AtomicU32 = AtomicU32::new(0);

#[unsafe(link_section = ".bss.stack")]
static mut VMM_BOOT_STACK: [[u8; TASK_STACK_SIZE]; SMP] = [[0; TASK_STACK_SIZE]; SMP];

pub fn entered_cpus() -> u32 {
    ENTERED_CPUS.load(Ordering::Acquire)
}

#[unsafe(link_section = ".text.boot")]
unsafe extern "sysv64" fn switch_stack(linux_sp: usize) -> i32 {
    unsafe {
        let linux_cr3 = x86::controlregs::cr3();
        let linux_tp = x86::msr::rdmsr(x86::msr::IA32_GS_BASE);

        let vmm_entry = |linux_sp: usize| -> i32 {
            if entered_cpus() >= HvHeader::get().max_cpus {
                panic!(
                    "enter cpus exceed Linux max cpus {}",
                    HvHeader::get().max_cpus
                );
            }
            if entered_cpus() >= SMP as u32 {
                panic!("enter cpus exceed configured SMP {}", SMP);
            }

            // Note: cpu_id here is not Local APIC ID, it is the index of entered CPUs.
            // We just use it here to choose VMM_BOOT_STACK.
            let core_id = ENTERED_CPUS.fetch_add(1, Ordering::SeqCst);
            // let _cpu_id = current_cpu_id();

            // let cpu_data = PerCpu::new();
            let hv_sp = VMM_BOOT_STACK[core_id as usize].as_ptr_range().end as usize;
            let ret;
            core::arch::asm!("
                mov [rsi], {linux_tp}   // save gs_base to stack
                mov rcx, rsp
                mov rsp, {hv_sp}
                push rcx
                call {entry}
                pop rsp",
                entry = sym super::vmm_cpu_entry,
                linux_tp = in(reg) linux_tp,
                hv_sp = in(reg) hv_sp,
                in("rdi") core_id,
                in("rsi") linux_sp,
                lateout("rax") ret,
                out("rcx") _,
                clobber_abi("sysv64"),
            );
            ret
        };

        let ret = vmm_entry(linux_sp);

        x86::msr::wrmsr(x86::msr::IA32_GS_BASE, linux_tp);
        x86::controlregs::cr3_write(linux_cr3);
        ret
    }
}

#[naked]
#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> i32 {
    unsafe {
        core::arch::naked_asm!("
        .code64
        // rip is pushed
        cli
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15
        push 0  // skip gs_base

        mov rdi, rsp
        call {0}

        pop r15 // skip gs_base
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        ret
        // rip will pop when return",
            sym switch_stack,
        );
    }
}
