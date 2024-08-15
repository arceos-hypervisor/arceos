use core::arch::global_asm;
use core::sync::atomic::{AtomicUsize, Ordering};
use tock_registers::interfaces::*;

use crate::arch::TrapFrame;

global_asm!(include_str!("trap.S"));

type VmExitHandler = unsafe extern "C" fn();

#[no_mangle]
static mut LOWER_AARCH64_SYNCHROUNOUS_HANDLER: VmExitHandler = dummy_vmexit_handler;

#[no_mangle]
static mut LOWER_AARCH64_IRQ_HANDLER: VmExitHandler = dummy_vmexit_handler;

unsafe extern "C" fn dummy_vmexit_handler() {}

#[allow(dead_code)]
pub unsafe fn register_lower_aarch64_synchronous_handler(handler: VmExitHandler) {
    LOWER_AARCH64_SYNCHROUNOUS_HANDLER = handler;
}

#[allow(dead_code)]
pub unsafe fn register_lower_aarch64_irq_handler(handler: VmExitHandler) {
    LOWER_AARCH64_IRQ_HANDLER = handler;
}

#[repr(u8)]
#[derive(Debug)]
#[allow(dead_code)]
enum TrapKind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u8)]
#[derive(Debug)]
#[allow(dead_code)]
enum TrapSource {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

/// deal with invalid aarch64 synchronous exception
#[no_mangle]
fn invalid_exception_el2(tf: &mut TrapFrame, kind: TrapKind, source: TrapSource) {
    panic!(
        "Invalid exception {:?} from {:?}:\n{:#x?}",
        kind, source, tf
    );
}

/// deal with current el irq exception (need to remove after implement interrupt virtualization)
#[no_mangle]
fn handle_irq_exception(_tf: &TrapFrame) {
    if !handle_trap!(IRQ, 0) {
        unsafe { LOWER_AARCH64_IRQ_HANDLER() };
    }
}
