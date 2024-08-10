use core::arch::global_asm;
use core::sync::atomic::{AtomicUsize, Ordering};

use aarch64_cpu::registers::{ESR_EL2, FAR_EL2};
use tock_registers::interfaces::*;

use memory_addr::VirtAddr;
use page_table_entry::MappingFlags;

use super::exception_utils::*;
use crate::arch::TrapFrame;

global_asm!(include_str!("exception.S"));

type ExceptionHandler = fn(&mut TrapFrame);

const EMPTY_HANDLER_VALUE: AtomicUsize = AtomicUsize::new(0);

static LOWER_AARCH64_HANDLERS: [AtomicUsize; MAX_EXCEPTION_COUNT] =
    [EMPTY_HANDLER_VALUE; MAX_EXCEPTION_COUNT];

const MAX_EXCEPTION_COUNT: usize = 64;

#[allow(dead_code)]
pub fn register_exception_handler_aarch64(
    exception_class: usize,
    handler: ExceptionHandler,
) -> bool {
    if exception_class < MAX_EXCEPTION_COUNT {
        LOWER_AARCH64_HANDLERS[exception_class].store(handler as usize, Ordering::SeqCst);
        return true;
    }
    return false;
}

fn call_handler(exception_class: usize, tf: &mut TrapFrame) -> bool {
    if exception_class < MAX_EXCEPTION_COUNT {
        let handler = LOWER_AARCH64_HANDLERS[exception_class].load(Ordering::Acquire);
        if handler != 0 {
            let handler: ExceptionHandler = unsafe { core::mem::transmute(handler) };
            handler(tf);
            return true;
        } else {
            return false;
        }
    } else {
        false
    }
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

fn handle_data_abort(tf: &TrapFrame, iss: u64, is_guest: bool) {
    let wnr = (iss & (1 << 6)) != 0; // WnR: Write not Read
    let cm = (iss & (1 << 8)) != 0; // CM: Cache maintenance
    let mut access_flags = if wnr & !cm {
        MappingFlags::WRITE
    } else {
        MappingFlags::READ
    };
    if is_guest {
        access_flags |= MappingFlags::USER;
    }
    let vaddr = VirtAddr::from(FAR_EL2.get() as usize);

    // Only handle Translation fault and Permission fault
    panic!(
        "Unhandled {} Data Abort @ {:#x}, fault_vaddr={:#x}, ISS=0b{:08b} ({:?}):\n{:#x?}",
        if is_guest { "EL1" } else { "EL2" },
        tf.elr,
        vaddr,
        iss,
        access_flags,
        tf,
    );
}

/// deal with invalid aarch64 synchronous exception
#[no_mangle]
fn invalid_exception_el2(tf: &mut TrapFrame, kind: TrapKind, source: TrapSource) {
    let esr = ESR_EL2.extract();
    let iss = esr.read(ESR_EL2::ISS);
    let ec = esr.read(ESR_EL2::EC);

    match esr.read_as_enum(ESR_EL2::EC) {
        Some(ESR_EL2::EC::Value::DataAbortLowerEL) => handle_data_abort(tf, iss, true),
        Some(ESR_EL2::EC::Value::DataAbortCurrentEL) => handle_data_abort(tf, iss, false),
        _ => {
            panic!(
                "Unhandled synchronous exception @ {:#x}: ESR={:#x} (EC {:#08b}, ISS {:#x})",
                tf.elr,
                esr.get(),
                esr.read(ESR_EL2::EC),
                esr.read(ESR_EL2::ISS),
            );
        }
    }
    warn!("iss {:#x} ec {:#x}", iss, ec);

    panic!(
        "Invalid exception {:?} from {:?}:\n{:#x?}",
        kind, source, tf
    );
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

/// deal with lower aarch64 synchronous exception
#[no_mangle]
fn lower_aarch64_synchronous(tf: &mut TrapFrame) {
    debug!(
        "enter lower_aarch64_synchronous exception class:0x{:X}",
        exception_class()
    );
    // 0x16: hvc_handler
    // 0x24: data_abort_handler
    call_handler(exception_class(), tf);
}
