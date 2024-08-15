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

#[allow(dead_code)]
fn get_lower_aarch64_irq_handler() -> VmExitHandler {
    unsafe { LOWER_AARCH64_IRQ_HANDLER }
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
        let handler = get_lower_aarch64_irq_handler();
        handler();   
    }
}

// /// deal with lower aarch64 interruption exception
// #[no_mangle]
// fn current_spxel_irq(ctx: &mut TrapFrame) {
//     lower_aarch64_irq(ctx);
// }

// /// deal with lower aarch64 interruption exception
// #[no_mangle]
// fn lower_aarch64_irq(ctx: &mut TrapFrame) {
//     let (irq, src) = gicc_get_current_irq();
//     debug!("src {} id{}", src, irq);
//     crate::trap::handle_irq_extern_hv(irq, src, ctx);
// }

// /// deal with lower aarch64 synchronous exception
// #[no_mangle]
// fn lower_aarch64_synchronous(tf: &mut TrapFrame) {
//     debug!(
//         "enter lower_aarch64_synchronous exception class:0x{:X}",
//         exception_class()
//     );
//     // 0x16: hvc_handler
//     // 0x24: data_abort_handler
//     call_handler(exception_class(), tf);
// }
