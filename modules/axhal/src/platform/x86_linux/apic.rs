#![allow(dead_code)]

use core::{cell::SyncUnsafeCell, mem::MaybeUninit};

use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use memory_addr::PhysAddr;
use x2apic::ioapic::IoApic;
use x2apic::lapic::{cpu_has_x2apic, xapic_base, LocalApic, LocalApicBuilder, LocalApicMode};
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

use self::vectors::*;
use crate::mem::phys_to_virt;

pub(super) mod vectors {
    pub const APIC_TIMER_VECTOR: u8 = 0xf0;
    pub const APIC_SPURIOUS_VECTOR: u8 = 0xf1;
    pub const APIC_ERROR_VECTOR: u8 = 0xf2;
}

/// The maximum number of IRQs.
pub const MAX_IRQ_COUNT: usize = 256;

/// The timer IRQ number.
pub const TIMER_IRQ_NUM: usize = APIC_TIMER_VECTOR as usize;

const IO_APIC_BASE: PhysAddr = pa!(0xFEC0_0000);

static LOCAL_APIC: SyncUnsafeCell<MaybeUninit<LocalApic>> =
    SyncUnsafeCell::new(MaybeUninit::uninit());
static mut IS_X2APIC: bool = false;
static IO_APIC: LazyInit<SpinNoIrq<IoApic>> = LazyInit::new();

const MAX_APIC_ID: u32 = 254;
static mut APIC_TO_CPU_ID: [u32; MAX_APIC_ID as usize + 1] = [u32::MAX; MAX_APIC_ID as usize + 1];
static mut APIC_ID_IS_RESERVED: [bool; MAX_APIC_ID as usize + 1] =
    [false; MAX_APIC_ID as usize + 1];

/// Enables or disables the given IRQ.
#[cfg(feature = "irq")]
pub fn set_enable(vector: usize, enabled: bool) {
    // should not affect LAPIC interrupts
    if vector < APIC_TIMER_VECTOR as _ {
        unsafe {
            if enabled {
                IO_APIC.lock().enable_irq(vector as u8);
            } else {
                IO_APIC.lock().disable_irq(vector as u8);
            }
        }
    }
}

/// Registers an IRQ handler for the given IRQ.
///
/// It also enables the IRQ if the registration succeeds. It returns `false` if
/// the registration failed.
#[cfg(feature = "irq")]
pub fn register_handler(vector: usize, handler: crate::irq::IrqHandler) -> bool {
    crate::irq::register_handler_common(vector, handler)
}

/// Dispatches the IRQ.
///
/// This function is called by the common interrupt handler. It looks
/// up in the IRQ handler table and calls the corresponding handler. If
/// necessary, it also acknowledges the interrupt controller after handling.
#[cfg(feature = "irq")]
pub fn dispatch_irq(vector: usize) {
    crate::irq::dispatch_irq_common(vector);
    unsafe { local_apic().end_of_interrupt() };
}

pub(super) fn local_apic<'a>() -> &'a mut LocalApic {
    // It's safe as `LOCAL_APIC` is initialized in `init_primary`.
    unsafe { LOCAL_APIC.get().as_mut().unwrap().assume_init_mut() }
}

pub(super) fn raw_apic_id(id_u8: u8) -> u32 {
    if unsafe { IS_X2APIC } {
        id_u8 as u32
    } else {
        (id_u8 as u32) << 24
    }
}

fn x2apic_enabled() -> bool {
    const XAPIC_ENABLE: u64 = 1 << 11;
    const X2APIC_ENABLE: u64 = 1 << 10;
    const IA32_APIC_BASE: u32 = 0x1B;

    let ia32_apic_base = unsafe { Msr::new(IA32_APIC_BASE).read() };
    ia32_apic_base & XAPIC_ENABLE != 0 && ia32_apic_base & X2APIC_ENABLE != 0
}

pub(super) fn init_primary(enabled: bool, core_id: usize) {
    info!("Core {core_id} Initialize Local APIC...");

    if enabled {
        unsafe {
            // Disable 8259A interrupt controllers
            Port::<u8>::new(0x21).write(0xff);
            Port::<u8>::new(0xA1).write(0xff);
        }
    }

    let mut builder = LocalApicBuilder::new();
    builder
        .timer_vector(APIC_TIMER_VECTOR as _)
        .error_vector(APIC_ERROR_VECTOR as _)
        .spurious_vector(APIC_SPURIOUS_VECTOR as _);

    let use_x2apic = if enabled {
        cpu_has_x2apic()
    } else {
        x2apic_enabled()
    };

    if use_x2apic {
        info!("Using x2APIC.");
        unsafe { IS_X2APIC = true };
    } else {
        info!("Using xAPIC.");
        let base_vaddr = phys_to_virt(PhysAddr::from(unsafe { xapic_base() } as usize));
        builder.apic_mode(LocalApicMode::XApic { xapic_base: base_vaddr.as_usize() as _ });
    }

    let mut lapic = builder.build().unwrap();
    unsafe {
        if enabled {
            lapic.enable();
        }

        let apic_id = lapic.id();
        APIC_TO_CPU_ID[apic_id as usize] = core_id as u32;
        if crate::cpu::this_cpu_is_reserved() {
            APIC_ID_IS_RESERVED[apic_id as usize] = true;
        }

        LOCAL_APIC.get().as_mut().unwrap().write(lapic);
    }

    // info!("Initialize IO APIC...");
    // let io_apic = unsafe { IoApic::new(phys_to_virt(IO_APIC_BASE).as_usize() as u64) };
    // IO_APIC.init_once(SpinNoIrq::new(io_apic));
}

#[cfg(feature = "smp")]
pub(super) fn init_secondary(enabled: bool, core_id: usize) {
    let lapic = local_apic();

    if enabled {
        unsafe {
            lapic.enable();
        }
    }
    unsafe {
        let apic_id = lapic.id();
        APIC_TO_CPU_ID[apic_id as usize] = core_id as u32;
        if crate::cpu::this_cpu_is_reserved() {
            APIC_ID_IS_RESERVED[apic_id as usize] = true;
        }
    };
}

/// Returns if the given APIC ID is reserved.
/// The APIC ID is reserved if it entered Linux, which has set the corresponding
/// entry in `APIC_TO_CPU_ID` to 0.
pub(super) fn apic_id_is_reserved(apic_id: usize) -> bool {
    unsafe { APIC_ID_IS_RESERVED[apic_id] }
}

pub(super) fn apic_to_cpu_id(apic_id: u32) -> u32 {
    if apic_id <= MAX_APIC_ID {
        unsafe { APIC_TO_CPU_ID[apic_id as usize] }
    } else {
        u32::MAX
    }
}

pub(super) fn cpu_id_to_apic_id(cpu_id: usize) -> Option<u32> {
    for (apic_id, id) in unsafe { APIC_TO_CPU_ID.iter().enumerate() } {
        if *id == cpu_id as u32 {
            return Some(apic_id as u32);
        }
    }
    None
}

pub(super) fn cpu_id_speculate_apic_id(cpu_id: usize) -> u32 {
    for (apic_id, id) in unsafe { APIC_TO_CPU_ID.iter().enumerate() } {
        if *id == cpu_id as u32 {
            return apic_id as u32;
        }
    }

    // Ouch, we have to speculate the APIC ID.
    // This is not a good idea, but we have no choice.

    let mut core_id_sum = 0;
    let mut apic_id_sum = 0;

    for core_id in 0..3 {
        match cpu_id_to_apic_id(core_id) {
            Some(apic_id) => {
                core_id_sum += core_id as u32;
                apic_id_sum += apic_id;
            }
            None => {}
        };
    }

    let apic_id = match apic_id_sum {
        0 => cpu_id as u32,
        _ => {
            let apic_id = (apic_id_sum / core_id_sum) * cpu_id as u32;
            if apic_id > MAX_APIC_ID {
                warn!("Speculated APIC ID {apic_id} is too large, using cpu_id {cpu_id}");
                cpu_id as u32
            } else {
                apic_id
            }
        }
    };

    warn!("Speculating APIC ID for CPU {cpu_id} as {apic_id}");

    apic_id
}

/// Shuts down the target CPU by sending an INIT IPI to it.
pub(super) fn shutdown_ap(apic_id: u32) {
    info!("Shutting down ArceOS cpu {apic_id}...");
    let apic_id = raw_apic_id(apic_id as u8);
    unsafe {
        local_apic().send_init_ipi(apic_id);
    }
}
