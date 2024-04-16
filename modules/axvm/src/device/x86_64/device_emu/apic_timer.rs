//! Emulated Local APIC. (SDM Vol. 3A, Chapter 10)

#![allow(dead_code)]
use crate::{Error as HyperError, Result as HyperResult};
use axhal::time::current_time_nanos;
use bit_field::BitField;

use super::{msr_proxy_struct, msr_proxy_factory};
use crate::device::VirtMsrOps;

const APIC_FREQ_MHZ: u64 = 1000; // 1000 MHz
const APIC_CYCLE_NANOS: u64 = 1000 / APIC_FREQ_MHZ;

/// Local APIC timer modes.
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
#[allow(dead_code)]
pub enum TimerMode {
    /// Timer only fires once.
    OneShot = 0b00,
    /// Timer fires periodically.
    Periodic = 0b01,
    /// Timer fires at an absolute time.
    TscDeadline = 0b10,
}

/// A virtual local APIC timer. (SDM Vol. 3C, Section 10.5.4)
pub struct ApicTimer {
    lvt_timer_bits: u32,
    divide_shift: u8,
    initial_count: u32,
    last_start_ns: u64,
    deadline_ns: u64,
    tpr: u32,
}

impl ApicTimer {
    pub(crate) const fn new() -> Self {
        Self {
            lvt_timer_bits: 0x1_0000, // masked
            divide_shift: 0,
            initial_count: 0,
            last_start_ns: 0,
            deadline_ns: 0,
            tpr: 0,
        }
    }

    /// Check if an interrupt generated. if yes, update it's states.
    pub fn check_interrupt(&mut self) -> bool {
        if self.deadline_ns == 0 {
            false
        } else if current_time_nanos() >= self.deadline_ns {
            if self.is_periodic() {
                self.deadline_ns += self.interval_ns();
            } else {
                self.deadline_ns = 0;
            }
            !self.is_masked()
        } else {
            false
        }
    }

    /// Whether the timer interrupt is masked.
    pub const fn is_masked(&self) -> bool {
        self.lvt_timer_bits & (1 << 16) != 0
    }

    /// Whether the timer mode is periodic.
    pub const fn is_periodic(&self) -> bool {
        let timer_mode = (self.lvt_timer_bits >> 17) & 0b11;
        timer_mode == TimerMode::Periodic as _
    }

    /// The timer interrupt vector number.
    pub const fn vector(&self) -> u8 {
        (self.lvt_timer_bits & 0xff) as u8
    }

    /// LVT Timer Register. (SDM Vol. 3A, Section 10.5.1, Figure 10-8)
    pub const fn lvt_timer(&self) -> u32 {
        self.lvt_timer_bits
    }

    /// Divide Configuration Register. (SDM Vol. 3A, Section 10.5.4, Figure 10-10)
    pub const fn divide(&self) -> u32 {
        let dcr = self.divide_shift.wrapping_sub(1) as u32 & 0b111;
        (dcr & 0b11) | ((dcr & 0b100) << 1)
    }

    /// Initial Count Register.
    pub const fn initial_count(&self) -> u32 {
        self.initial_count
    }

    /// Current Count Register.
    pub fn current_counter(&self) -> u32 {
        let elapsed_ns = current_time_nanos() - self.last_start_ns;
        let elapsed_cycles = (elapsed_ns / APIC_CYCLE_NANOS) >> self.divide_shift;
        if self.is_periodic() {
            self.initial_count - (elapsed_cycles % self.initial_count as u64) as u32
        } else if elapsed_cycles < self.initial_count as u64 {
            self.initial_count - elapsed_cycles as u32
        } else {
            0
        }
    }

    /// Set LVT Timer Register.
    pub fn set_lvt_timer(&mut self, bits: u32) -> HyperResult {
        let timer_mode = bits.get_bits(17..19);
        if timer_mode == TimerMode::TscDeadline as _ {
            return Err(HyperError::NotSupported); // TSC deadline mode was not supported
        } else if timer_mode == 0b11 {
            return Err(HyperError::InvalidParam); // reserved
        }
        self.lvt_timer_bits = bits;
        self.start_timer();
        Ok(())
    }

    /// Set Initial Count Register.
    pub fn set_initial_count(&mut self, initial: u32) -> HyperResult {
        self.initial_count = initial;
        self.start_timer();
        Ok(())
    }

    /// Set Divide Configuration Register.
    pub fn set_divide(&mut self, dcr: u32) -> HyperResult {
        let shift = (dcr & 0b11) | ((dcr & 0b1000) >> 1);
        self.divide_shift = (shift + 1) as u8 & 0b111;
        self.start_timer();
        Ok(())
    }

    const fn interval_ns(&self) -> u64 {
        (self.initial_count as u64 * APIC_CYCLE_NANOS) << self.divide_shift
    }

    fn start_timer(&mut self) {
        if self.initial_count != 0 {
            self.last_start_ns = current_time_nanos();
            self.deadline_ns = self.last_start_ns + self.interval_ns();
        } else {
            self.deadline_ns = 0;
        }
    }

    pub fn tpr(&self) -> u32 {
        self.tpr
    }

    pub fn set_tpr(&mut self, value: u32) {
        self.tpr = value;
    }
}

/// ID register.
const APICID: u32 = 0x2;
/// Version register.
const VERSION: u32 = 0x3;
/// Task priority register.
const TPR: u32 = 0x8;
/// EOI register.
const EOI: u32 = 0xB;
/// Logical Destination Register.
const LDR: u32 = 0xD;
/// Spurious Interrupt Vector register.
const SIVR: u32 = 0xF;
/// In-Service Register 0
const ISR0: u32 = 0x10;
/// In-Service Register 1
const ISR1: u32 = 0x11;
/// In-Service Register 2
const ISR2: u32 = 0x12;
/// In-Service Register 3
const ISR3: u32 = 0x13;
/// In-Service Register 4
const ISR4: u32 = 0x14;
/// In-Service Register 5
const ISR5: u32 = 0x15;
/// In-Service Register 6
const ISR6: u32 = 0x16;
/// In-Service Register 7
const ISR7: u32 = 0x17;
/// Interrupt Request Register 0
const IRR0: u32 = 0x20;
/// Interrupt Request Register 1
const IRR1: u32 = 0x21;
/// Interrupt Request Register 2
const IRR2: u32 = 0x22;
/// Interrupt Request Register 3
const IRR3: u32 = 0x23;
/// Interrupt Request Register 4
const IRR4: u32 = 0x24;
/// Interrupt Request Register 5
const IRR5: u32 = 0x25;
/// Interrupt Request Register 6
const IRR6: u32 = 0x26;
/// Interrupt Request Register 7
const IRR7: u32 = 0x27;
/// Error Status Register.
const ESR: u32 = 0x28;
/// Interrupt Command register.
const ICR: u32 = 0x30;
/// LVT Timer Interrupt register.
const LVT_TIMER: u32 = 0x32;
/// LVT Thermal Sensor Interrupt register.
const LVT_THERMAL: u32 = 0x33;
/// LVT Performance Monitor register.
const LVT_PMI: u32 = 0x34;
/// LVT LINT0 register.
const LVT_LINT0: u32 = 0x35;
/// LVT LINT1 register.
const LVT_LINT1: u32 = 0x36;
/// LVT Error register.
const LVT_ERR: u32 = 0x37;
///  Initial Count register.
const INIT_COUNT: u32 = 0x38;
/// Current Count register.
const CUR_COUNT: u32 = 0x39;
/// Divide Configuration register.
const DIV_CONF: u32 = 0x3E;

pub struct VirtLocalApic {
    pub inner: ApicTimer,
}

msr_proxy_struct!(
    0x800,
    0x83f,
    VirtLocalApicMsrProxy,
    VirtLocalApic,
    read_msr,
    write_msr
);

impl VirtLocalApic {
    pub fn new() -> Self {
        Self {
            inner: ApicTimer::new(),
        }
    }

    pub const fn msr_range() -> core::ops::Range<u32> {
        0x800..0x840
    }

    fn read_msr(&mut self, msr: u32) -> HyperResult<u64> {
        let apic_timer = &mut self.inner;
        let offset = msr - 0x800;
        match offset {
            SIVR => Ok(0x1ff), // SDM Vol. 3A, Section 10.9, Figure 10-23 (with Software Enable bit)
            LVT_THERMAL | LVT_PMI | LVT_LINT0 | LVT_LINT1 | LVT_ERR => {
                Ok(0x1_0000) // SDM Vol. 3A, Section 10.5.1, Figure 10-8 (with Mask bit)
            }
            IRR0..=IRR7 => Ok(0),
            ISR0..=ISR7 => Ok(0),
            LVT_TIMER => Ok(apic_timer.lvt_timer() as u64),
            INIT_COUNT => Ok(apic_timer.initial_count() as u64),
            DIV_CONF => Ok(apic_timer.divide() as u64),
            CUR_COUNT => Ok(apic_timer.current_counter() as u64),
            LDR => Ok(0),
            TPR => Ok(apic_timer.tpr() as u64),
            VERSION => Ok(0b0000000_0_00000000_00000110_00010101), // Suppress EOI-broadcasts: false, Max LVT Entry: 6, Version: 0x15
            ESR => Ok(0),
            APICID => Ok(0),
            _ => Err(HyperError::NotSupported),
        }
    }

    fn write_msr(&mut self, msr: u32, value: u64) -> HyperResult {
        let apic_timer = &mut self.inner;
        let offset = msr - 0x800;

        if offset != ICR && (value >> 32) != 0 {
            return Err(HyperError::InvalidParam); // all registers except ICR are 32-bits
        }
        match offset {
            EOI => {
                if value != 0 {
                    Err(HyperError::InvalidParam) // write a non-zero value causes #GP
                } else {
                    Ok(())
                }
            }
            SIVR | LVT_THERMAL | LVT_PMI | LVT_LINT0 | LVT_LINT1 | LVT_ERR => {
                Ok(()) // ignore these register writes
            }
            LVT_TIMER => apic_timer.set_lvt_timer(value as u32),
            INIT_COUNT => apic_timer.set_initial_count(value as u32),
            DIV_CONF => apic_timer.set_divide(value as u32),
            TPR => Ok(apic_timer.set_tpr(value as u32)),
            ESR => {
                if value == 0 {
                    Ok(())
                } else {
                    Err(HyperError::InvalidParam)
                }
            }
            _ => Err(HyperError::NotSupported),
        }
    }

    msr_proxy_factory!(msr_proxy, VirtLocalApicMsrProxy);
}

pub struct ApicBaseMsrHandler;

impl ApicBaseMsrHandler {
    pub fn new() -> Self {
        Self
    }
}

impl VirtMsrOps for ApicBaseMsrHandler {
    fn msr_range(&self) -> core::ops::Range<u32> {
        x86::msr::IA32_APIC_BASE..(x86::msr::IA32_APIC_BASE + 1)
    }

    fn read(&mut self, msr: u32) -> HyperResult<u64> {
        let _ = msr;
        let mut apic_base = unsafe { x86::msr::rdmsr(x86::msr::IA32_APIC_BASE) };
        apic_base |= 1 << 11 | 1 << 10; // enable xAPIC and x2APIC
        Ok(apic_base)
    }

    fn write(&mut self, _msr: u32, _value: u64) -> HyperResult {
        Ok(())
    }
}
