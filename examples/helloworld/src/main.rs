#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(naked_functions)]

#[cfg(feature = "axstd")]
use axstd::println;

#[cfg(target_arch = "x86_64")]
#[naked]
unsafe extern "C" fn test_guest() -> ! {
    core::arch::asm!(
        "
        mov     rax, 0
        mov     rdi, 2
        mov     rsi, 3
        mov     rdx, 3
        mov     rcx, 3
    2:
        vmcall
        add     rax, 1
        jmp     2b",
        options(noreturn),
    );
}

#[cfg(target_arch = "aarch64")]
#[naked]
unsafe extern "C" fn test_guest() -> ! {
    core::arch::asm!(
        "
        mov     x0, 0
        mov     x1, 2
        mov     x2, 3
        mov     x3, 3
        mov     x4, 3
    2:
        hvc     #0
        add     x0, x0, 1
        b     2b",
        options(noreturn),
    );
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("Hello, world!");
    unsafe { test_guest() }
}
