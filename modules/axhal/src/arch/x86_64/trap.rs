use x86::{controlregs::cr2, irq::*};

use crate::current_cpu_id;

use super::context::TrapFrame;

core::arch::global_asm!(include_str!("trap.S"));

pub const SYSCALL_VECTOR: u8 = 0x80;
pub const NMI_VECTOR: u8 = 0x2;

const IRQ_VECTOR_START: u8 = 0x20;
const IRQ_VECTOR_END: u8 = 0xff;

#[no_mangle]
fn x86_trap_handler(tf: &mut TrapFrame) {
    match tf.vector as u8 {
        PAGE_FAULT_VECTOR => {
            #[cfg(feature = "monolithic")]
            if tf.is_user() {
                panic!(
                    "User #PF @ {:#x}, fault_vaddr={:#x}, error_code={:#x}",
                    tf.rip,
                    unsafe { cr2() },
                    tf.error_code,
                );
            }
            panic!(
                "Kernel #PF @ {:#x}, fault_vaddr={:#x}, error_code={:#x}:\n{:#x?}",
                tf.rip,
                unsafe { cr2() },
                tf.error_code,
                tf,
            );
        }
        NMI_VECTOR => {
            // warn!(
            //     "Unhandled NMI exception {} on Core {} (error_code = {:#x}) @ {:#x}:\n{:#x?}",
            //     tf.vector,
            //     current_cpu_id(),
            //     tf.error_code,
            //     tf.rip,
            //     tf
            // );
            // let value = unsafe { x86::io::inb(0x92) };
            // warn!("System Control Port A value {:#x}", value);
            // let value = unsafe { x86::io::inb(0x61) };
            // warn!("System Control Port B value {:#x}", value);
        }
        BREAKPOINT_VECTOR => debug!("#BP @ {:#x} ", tf.rip),
        GENERAL_PROTECTION_FAULT_VECTOR => {
            panic!(
                "#GP @ {:#x}, error_code={:#x}:\n{:#x?}",
                tf.rip, tf.error_code, tf
            );
        }
        SYSCALL_VECTOR => {
            debug!(
                "SYSCALL_VECTOR @ {:#x}, rax {:#x}, rdi {:#x} rsi {:#x} rdx {:#x}",
                tf.rip, tf.rax, tf.rdi, tf.rsi, tf.rdx,
            );
            tf.rax = 0;
            // tf.rax = syscall(tf, tf.rax as _, tf.rdi as _, tf.rsi as _, tf.rdx as _) as u64
        }
        IRQ_VECTOR_START..=IRQ_VECTOR_END => {
            debug!(
                "IRQ @ {:#x}, rax {:#x}, rdi {:#x} rsi {:#x} rdx {:#x}",
                tf.rip, tf.rax, tf.rdi, tf.rsi, tf.rdx,
            );
            crate::trap::handle_irq_extern(tf.vector as _);
        }
        _ => {
            panic!(
                "Unhandled exception {} (error_code = {:#x}) @ {:#x}:\n{:#x?}",
                tf.vector, tf.error_code, tf.rip, tf
            );
        }
    }
}
