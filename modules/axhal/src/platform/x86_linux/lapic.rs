/// This is just a simplified LocalApic used for send sipi request to other cores.
/// Refer to `apic.rs` for detailed implementation for APIC.
// Because `LocalApicBuilder` from `x2apic` can not be used here.
use spin::{Once, RwLock};
use x86::apic::{ApicControl, ApicId, x2apic::X2APIC, xapic::XAPIC};

extern crate alloc;

/// TODO: remove this Arc.
use alloc::sync::Arc;

use crate::mem::{PhysAddr, phys_to_virt};

const APIC_BASE: PhysAddr = pa!(0xFEE0_0000);

bitflags::bitflags! {
    /// IA32_APIC_BASE MSR.
    struct ApicBase: u64 {
        /// Processor is BSP.
        const BSP   = 1 << 8;
        /// Enable x2APIC mode.
        const EXTD  = 1 << 10;
        /// xAPIC global enable/disable.
        const EN    = 1 << 11;
    }
}

impl ApicBase {
    pub fn read() -> Self {
        unsafe { Self::from_bits_retain(x86::msr::rdmsr(x86::msr::IA32_APIC_BASE)) }
    }
}

pub(super) struct LocalApic {
    inner: Arc<RwLock<dyn ApicControl>>,
    is_x2apic: bool,
}

unsafe impl Send for LocalApic {}
unsafe impl Sync for LocalApic {}

impl LocalApic {
    pub fn new() -> Result<Self, &'static str> {
        let base = ApicBase::read();

        debug!("ApicBase {:#x}", base.bits());

        if base.contains(ApicBase::EXTD) {
            info!("Using x2APIC.");
            Ok(Self {
                inner: Arc::new(RwLock::new(X2APIC::new())),
                is_x2apic: true,
            })
        } else if base.contains(ApicBase::EN) {
            info!("Using xAPIC.");
            let base_vaddr = phys_to_virt(APIC_BASE);
            let apic_region =
                unsafe { core::slice::from_raw_parts_mut(base_vaddr.as_usize() as _, 0x1000 / 4) };
            Ok(Self {
                inner: Arc::new(RwLock::new(XAPIC::new(apic_region))),
                is_x2apic: false,
            })
        } else {
            Err("Local Apic init failed")
        }
    }
}

static LOCAL_APIC: Once<LocalApic> = Once::new();

pub(super) fn lapic<'a>() -> &'a LocalApic {
    LOCAL_APIC.get().expect("Uninitialized Local APIC!")
}

pub(super) fn init() {
    let lapic = LocalApic::new().expect("LocalApic init failed");
    LOCAL_APIC.call_once(|| lapic);
}

use crate::time::{Duration, busy_wait};

pub(super) unsafe fn start_ap(apic_id: u32, start_page_idx: u8) {
    info!("Starting RT cpu {}...", apic_id);
    let apic_id = if lapic().is_x2apic {
        ApicId::X2Apic(apic_id)
    } else {
        ApicId::XApic(apic_id as u8)
    };

    // INIT-SIPI-SIPI Sequence
    let mut lapic = lapic().inner.write();
    unsafe {
        lapic.ipi_init(apic_id);
    }
    // delay_us(10 * 1000); // 10ms
    busy_wait(Duration::from_millis(10)); // 10ms
    unsafe {
        lapic.ipi_startup(apic_id, start_page_idx);
    }
    // delay_us(200); // 200 us
    busy_wait(Duration::from_micros(200)); // 200us
    unsafe {
        lapic.ipi_startup(apic_id, start_page_idx);
    }
}

pub(super) unsafe fn shutdown_ap(apic_id: u32) {
    info!("Shutting down RT cpu {}...", apic_id);
    let apic_id = if lapic().is_x2apic {
        ApicId::X2Apic(apic_id)
    } else {
        ApicId::XApic(apic_id as u8)
    };
    unsafe {
        lapic().inner.write().ipi_init(apic_id);
    }
}
