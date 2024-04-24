#![allow(dead_code)]

use bit_field::BitField;
use lazy_init::LazyInit;
use memory_addr::PhysAddr;
use spinlock::SpinNoIrq;
use x2apic::ioapic::IoApic;
use x2apic::lapic::{xapic_base, LocalApic, LocalApicBuilder};

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

const IO_APIC_BASE: PhysAddr = PhysAddr::from(0xFEC0_0000);

static mut LOCAL_APIC: Option<LocalApic> = None;
static mut IS_X2APIC: bool = false;
static IO_APIC: LazyInit<SpinNoIrq<IoApic>> = LazyInit::new();

#[repr(C)]
pub struct ApicIcr {
    full: u64,
}

impl From<u64> for ApicIcr {
    fn from(item: u64) -> Self {
        ApicIcr { full: item }
    }
}

impl From<ApicIcr> for u64 {
    fn from(item: ApicIcr) -> Self {
        item.full
    }
}

impl ApicIcr {
    pub fn new(value: u64) -> Self {
        ApicIcr { full: value }
    }
    /// Get u64 value of icr
    pub fn value(&self) -> u64 {
        self.full
    }
    /// Get icr vector 0..7
    pub fn vector(&self) -> u8 {
        self.full.get_bits(0..8) as u8
    }
    /// Set icr vector 0..7
    pub fn set_vector(&mut self, value: u8) {
        self.full.set_bits(0..8, value as u64);
    }
    /// Get icr delivery mode 8..10
    pub fn delivery_mode(&self) -> u8 {
        self.full.get_bits(8..11) as u8
    }
    /// Set icr delivery mode 8..10
    pub fn set_delivery_mode(&mut self, value: u8) {
        self.full.set_bits(8..11, value as u64);
    }
    /// Get icr destination mode 11
    pub fn destination_mode(&self) -> bool {
        self.full.get_bit(11)
    }
    /// Set icr destination mode 11
    pub fn set_destination_mode(&mut self, value: bool) {
        self.full.set_bit(11, value);
    }
    /// Get icr reserved 12..13
    pub fn rsvd_1(&self) -> u8 {
        self.full.get_bits(12..14) as u8
    }
    /// Get icr level 14
    pub fn level(&self) -> bool {
        self.full.get_bit(14)
    }
    /// Set icr level 14
    pub fn set_level(&mut self, value: bool) {
        self.full.set_bit(14, value);
    }
    /// Get icr trigger mode 15
    pub fn trigger_mode(&self) -> bool {
        self.full.get_bit(15)
    }
    /// Set icr trigger mode 15
    pub fn set_trigger_mode(&mut self, value: bool) {
        self.full.set_bit(15, value);
    }
    /// Get icr rsvd 16..17
    pub fn rsvd_2(&self) -> u8 {
        self.full.get_bits(16..18) as u8
    }
    /// Get icr shorthand 18..19
    pub fn shorthand(&self) -> u8 {
        self.full.get_bits(18..20) as u8
    }
    /// Set icr shorthand 18..19
    pub fn set_shorthand(&mut self, value: u8) {
        self.full.set_bits(18..20, value as u64);
    }
    /// Get icr reserved 20..31
    pub fn rsvd_3(&self) -> u16 {
        self.full.get_bits(20..32) as u16
    }
    /// Get icr dest field 32..63
    pub fn dest_field(&self) -> u32 {
        self.full.get_bits(32..64) as u32
    }
    /// Set icr dest field 32..63
    pub fn set_dest_field(&mut self, value: u32) {
        self.full.set_bits(32..64, value as u64);
    }
}

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

pub fn send_ipi(irq_num: usize) {
    let mut io_apic = IO_APIC.lock();

    let entry = unsafe { io_apic.table_entry(irq_num as _) };
    let vector = entry.vector();
    let dest = entry.dest();

    if vector >= 0x20 {
        debug!("send_ipi {} {}", vector, dest);
        unsafe { local_apic().send_ipi(vector, dest as _) };
    }
}

pub fn send_nmi_to(dest: usize) {
    // unsafe{ local_apic().send_ipi(APIC_NMI_VECTOR as _, dest as _) };
    unsafe { local_apic().send_nmi(dest as _) };
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
    // It's safe as LAPIC is per-cpu.
    unsafe { LOCAL_APIC.as_mut().unwrap() }
}

pub(super) fn raw_apic_id(id_u8: u8) -> u32 {
    if unsafe { IS_X2APIC } {
        id_u8 as u32
    } else {
        (id_u8 as u32) << 24
    }
}

fn cpu_has_x2apic() -> bool {
    match raw_cpuid::CpuId::new().get_feature_info() {
        Some(finfo) => finfo.has_x2apic(),
        None => false,
    }
}

#[cfg(not(feature = "type1_5"))]
pub(super) fn init_primary() {
    info!("Initialize Local APIC...");

    unsafe {
        // Disable 8259A interrupt controllers
        Port::<u8>::new(0x21).write(0xff);
        Port::<u8>::new(0xA1).write(0xff);
    }

    let mut builder = LocalApicBuilder::new();
    builder
        .timer_vector(APIC_TIMER_VECTOR as _)
        .error_vector(APIC_ERROR_VECTOR as _)
        .spurious_vector(APIC_SPURIOUS_VECTOR as _);

    if cpu_has_x2apic() {
        info!("Using x2APIC.");
        unsafe { IS_X2APIC = true };
    } else {
        info!("Using xAPIC.");
        let base_vaddr = phys_to_virt(PhysAddr::from(unsafe { xapic_base() } as usize));
        builder.set_xapic_base(base_vaddr.as_usize() as u64);
    }

    let mut lapic = builder.build().unwrap();
    unsafe {
        lapic.enable();
        LOCAL_APIC = Some(lapic);
    }

    info!("Initialize IO APIC...");
    let io_apic = unsafe { IoApic::new(phys_to_virt(IO_APIC_BASE).as_usize() as u64) };
    IO_APIC.init_by(SpinNoIrq::new(io_apic));
}

#[cfg(feature = "type1_5")]
pub(super) fn init_primary() {
    info!("Type1.5 Initialize Local APIC...");

    let mut builder = LocalApicBuilder::new();
    builder
        .timer_vector(APIC_TIMER_VECTOR as _)
        .error_vector(APIC_ERROR_VECTOR as _)
        .spurious_vector(APIC_SPURIOUS_VECTOR as _);

    if cpu_has_x2apic() {
        info!("Using x2APIC.");
        unsafe { IS_X2APIC = true };
    } else {
        info!("Using xAPIC.");
        let base_vaddr = phys_to_virt(PhysAddr::from(unsafe { xapic_base() } as usize));
        builder.set_xapic_base(base_vaddr.as_usize() as u64);
    }
    let lapic = builder.build().unwrap();
    unsafe {
        LOCAL_APIC = Some(lapic);
    }

    info!("Type1.5 Initialize IO APIC...");
    let io_apic = unsafe { IoApic::new(phys_to_virt(IO_APIC_BASE).as_usize() as u64) };
    IO_APIC.init_by(SpinNoIrq::new(io_apic));
}

#[cfg(feature = "smp")]
pub(super) fn init_secondary() {
    unsafe { local_apic().enable() };
}
