use core::fmt::{Debug, Formatter, Result};
use core::sync::atomic::{AtomicU32, Ordering};

// use crate::arch::vmm::{Vcpu, VcpuAccessGuestState};
// use crate::arch::{cpu, ArchPerCpu, LinuxContext};
// use crate::cell::Cell;
use super::consts::{PER_CPU_ARRAY_PTR, PER_CPU_SIZE};
use super::current_cpu_id;
// use crate::error::HvResult;
use super::header::HvHeader;

static ENTERED_CPUS: AtomicU32 = AtomicU32::new(0);
// static ACTIVATED_CPUS: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Eq, PartialEq)]
pub enum CpuState {
    HvDisabled,
    HvEnabled,
}

static mut BOOTED_CPU_APIC_ID: [u32; 255] = [u32::MAX; 255];

// Todo: this can be placed into per_cpu.
#[repr(C, align(4096))]
pub struct PerCpu {
    /// Referenced by arch::cpu::thread_pointer() for x86_64.
    self_vaddr: usize,
    /// Current CPU's apic id, read from raw_cpuid.
    pub id: u32,
    pub state: CpuState,
    // pub vcpu: Vcpu,
    // arch: ArchPerCpu,
    // linux: LinuxContext,
    // Stack will be placed here.
}

impl PerCpu {
    pub fn new<'a>() -> &'a mut Self {
        if Self::entered_cpus() >= HvHeader::get().max_cpus {
            panic!("enter cpus exceed {}", HvHeader::get().max_cpus);
        }

        let cpu_sequence = ENTERED_CPUS.fetch_add(1, Ordering::SeqCst);
        let cpu_id = current_cpu_id();

        unsafe {
            BOOTED_CPU_APIC_ID[cpu_id] = cpu_sequence;
        }

        let ret = unsafe { Self::from_id_mut(cpu_id as u32) };
        let vaddr = ret as *const _ as usize;
        ret.id = cpu_id as u32;
        ret.self_vaddr = vaddr;

        // unsafe { crate::arch::write_thread_pointer(vaddr.into()) };
        ret
    }

    pub unsafe fn from_id_mut<'a>(cpu_id: u32) -> &'a mut Self {
        let vaddr = PER_CPU_ARRAY_PTR as usize + cpu_id as usize * PER_CPU_SIZE;
        &mut *(vaddr as *mut Self)
    }

    pub fn current<'a>() -> &'a Self {
        Self::current_mut()
    }

    pub fn current_mut<'a>() -> &'a mut Self {
        unsafe { &mut *(crate::arch::read_thread_pointer() as *mut Self) }
    }

    pub fn stack_top(&self) -> usize {
        self as *const _ as usize + PER_CPU_SIZE - 8
    }

    pub fn entered_cpus() -> u32 {
        ENTERED_CPUS.load(Ordering::Acquire)
    }

    // pub fn activated_cpus() -> u32 {
    //     ACTIVATED_CPUS.load(Ordering::Acquire)
    // }

    pub fn cpu_is_booted(apic_id: usize) -> bool {
        unsafe { BOOTED_CPU_APIC_ID[apic_id] != u32::MAX }
    }

    // pub fn init(&mut self, linux_sp: usize, cell: &Cell) -> HvResult {
    //     info!("CPU {} init...", self.id);

    //     // Save CPU state used for linux.
    //     self.state = CpuState::HvDisabled;
    //     self.linux = LinuxContext::load_from(linux_sp);

    //     // Activate hypervisor page table on each cpu.
    //     unsafe { crate::memory::hv_page_table().read().activate() };

    //     self.arch.init(self.id)?;

    //     // Initialize vCPU. Use `ptr::write()` to avoid dropping
    //     unsafe { core::ptr::write(&mut self.vcpu, Vcpu::new(&self.linux, cell)?) };

    //     self.state = CpuState::HvEnabled;
    //     Ok(())
    // }

    // pub fn activate_vmm(&mut self) -> HvResult {
    //     println!("Activating hypervisor on CPU {}...", self.id);
    //     ACTIVATED_CPUS.fetch_add(1, Ordering::SeqCst);

    //     self.vcpu.enter(&self.linux)?;
    //     unreachable!()
    // }

    // pub fn deactivate_vmm(&mut self, ret_code: usize) -> HvResult {
    //     println!("Deactivating hypervisor on CPU {}...", self.id);
    //     ACTIVATED_CPUS.fetch_sub(1, Ordering::SeqCst);

    //     self.vcpu.set_return_val(ret_code);
    //     self.vcpu.exit(&mut self.linux)?;
    //     self.linux.restore();
    //     self.state = CpuState::HvDisabled;
    //     self.linux.return_to_linux(self.vcpu.regs());
    // }

    // pub fn fault(&mut self) -> HvResult {
    //     warn!("VCPU fault: {:#x?}", self);
    //     self.vcpu.inject_fault()?;
    //     Ok(())
    // }
}

impl Debug for PerCpu {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let mut res = f.debug_struct("PerCpu");
        res.field("id", &self.id)
            .field("self_vaddr", &self.self_vaddr)
            .field("state", &self.state);
        // if self.state != CpuState::HvDisabled {
        //     res.field("vcpu", &self.vcpu);
        // } else {
        //     res.field("linux", &self.linux);
        // }
        res.finish()
    }
}
