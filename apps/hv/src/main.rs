#![no_std]
#![no_main]

extern crate alloc;
#[macro_use]
extern crate libax;

#[cfg(target_arch = "riscv64")]
use dtb_riscv64::MachineMeta;
#[cfg(target_arch = "aarch64")]
use dtb_aarch64::MachineMeta;
#[cfg(target_arch = "aarch64")]
use aarch64_config::GUEST_KERNEL_BASE_VADDR;
#[cfg(target_arch = "aarch64")]
use libax::{
    hv::{
        self, GuestPageTable, GuestPageTableTrait, HyperCraftHalImpl, PerCpu,
        Result, VCpu, VmCpus, VM,
    },
    info,
};
#[cfg(not(target_arch = "aarch64"))]
use libax::{
    hv::{
        self, GuestPageTable, GuestPageTableTrait, HyperCallMsg, HyperCraftHalImpl, PerCpu, Result,
        VCpu, VmCpus, VmExitInfo, VM, phys_to_virt,
    },
    info,
};

use page_table_entry::MappingFlags;

use vm_config::create_default_vm_config_entry;
#[cfg(target_arch = "x86_64")]
use device::{X64VcpuDevices, X64VmDevices};

#[cfg(target_arch = "riscv64")]
mod dtb_riscv64;
#[cfg(target_arch = "aarch64")]
mod dtb_aarch64;
#[cfg(target_arch = "aarch64")]
mod aarch64_config;

#[cfg(target_arch = "x86_64")]
mod x64;

#[cfg(target_arch = "x86_64")]
#[path = "device/x86_64/mod.rs"]
mod device;

#[cfg(not(target_arch = "x86_64"))]
#[path = "device/dummy.rs"]
mod device;

#[no_mangle]
fn main(hart_id: usize) {
    println!("Hello, hv!");

    #[cfg(target_arch = "riscv64")]
    {
        // boot cpu
        PerCpu::<HyperCraftHalImpl>::init(0, 0x4000);

        // get current percpu
        let pcpu = PerCpu::<HyperCraftHalImpl>::this_cpu();

        // create vcpu
        let gpt = setup_gpm(0x9000_0000).unwrap();
        let vcpu = pcpu.create_vcpu(0, 0x9020_0000).unwrap();
        let mut vcpus = VmCpus::new();

        // add vcpu into vm
        vcpus.add_vcpu(vcpu).unwrap();
        let mut vm: VM<HyperCraftHalImpl, GuestPageTable> = VM::new(vcpus, gpt).unwrap();
        vm.init_vcpu(0);

        // vm run
        info!("vm run cpu{}", hart_id);
        vm.run(0);
    }
    #[cfg(target_arch = "aarch64")]
    {
        // boot cpu
        PerCpu::<HyperCraftHalImpl>::init(0, 0x4000);   // change to pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128?

        // get current percpu
        let pcpu = PerCpu::<HyperCraftHalImpl>::this_cpu();

        // create vcpu, need to change addr for aarch64!
        let gpt = setup_gpm(0x7000_0000, 0x7020_0000).unwrap();  
        let vcpu = pcpu.create_vcpu(0).unwrap();
        let mut vcpus = VmCpus::new();

        // add vcpu into vm
        vcpus.add_vcpu(vcpu).unwrap();
        let mut vm: VM<HyperCraftHalImpl, GuestPageTable> = VM::new(vcpus, gpt, 0).unwrap();
        vm.init_vm_vcpu(0, 0x7020_0000, 0x7000_0000);

        info!("vm run cpu{}", hart_id);
        // suppose hart_id to be 0
        vm.run(0);
    }
    #[cfg(target_arch = "x86_64")]
    {
        println!("into main {}", hart_id);
        
        let vm_config = create_default_vm_config_entry();
        let image_config = vm_config.image.clone();

        let mut p = PerCpu::<HyperCraftHalImpl>::new(hart_id);
        p.hardware_enable().unwrap();

        let gpm = x64::setup_gpm(hart_id, vm_config.clone()).unwrap();
        let npt = gpm.nest_page_table_root();
        info!("setup gpm: {:#x?}", gpm);

        let mut vcpus = VmCpus::<HyperCraftHalImpl, X64VcpuDevices<HyperCraftHalImpl>>::new();
        // vcpus.add_vcpu(VCpu::new(0, p.vmcs_revision_id(), 0x7c00, npt).unwrap());
        vcpus.add_vcpu(VCpu::new(0, p.vmcs_revision_id(), image_config.bios_entry, npt).unwrap(), vm_config.clone());

        let mut vm = VM::<HyperCraftHalImpl, X64VcpuDevices<HyperCraftHalImpl>, X64VmDevices<HyperCraftHalImpl>>::new(vcpus);
        // TODO: according to config to add more vcpu
        vm.bind_vcpu(0);

        if hart_id == 0 {
            let (_, dev) = vm.get_vcpu_and_device(0).unwrap();
            *(dev.console.lock().backend()) = device::device_emu::MultiplexConsoleBackend::Primary;

            for v in 0..256 {
                libax::hv::set_host_irq_enabled(v, true);
            }
        }

        println!("Running guest...");
        println!("{:?}", vm.run_vcpu(0));

        p.hardware_disable().unwrap();

        panic!("done");

        return;
    }
    #[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64", target_arch = "aarch64")))]
    {
        panic!("Other arch is not supported yet!")
    }
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub fn main_secondary(hart_id: usize) {
    println!("secondary into main {}", hart_id);

    main(1);

    /*
    loop {
        libax::thread::sleep(libax::time::Duration::from_secs(5));
        println!("secondary tick");
    } */
}

#[cfg(target_arch = "riscv64")]
pub fn setup_gpm(dtb: usize) -> Result<GuestPageTable> {
    let mut gpt = GuestPageTable::new()?;
    let meta = MachineMeta::parse(dtb);
    if let Some(test) = meta.test_finisher_address {
        gpt.map_region(
            test.base_address,
            test.base_address,
            test.size + 0x1000,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER | MappingFlags::EXECUTE,
        )?;
    }
    for virtio in meta.virtio.iter() {
        gpt.map_region(
            virtio.base_address,
            virtio.base_address,
            virtio.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    if let Some(uart) = meta.uart {
        gpt.map_region(
            uart.base_address,
            uart.base_address,
            0x1000,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    if let Some(clint) = meta.clint {
        gpt.map_region(
            clint.base_address,
            clint.base_address,
            clint.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    if let Some(plic) = meta.plic {
        gpt.map_region(
            plic.base_address,
            plic.base_address,
            0x20_0000,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    if let Some(pci) = meta.pci {
        gpt.map_region(
            pci.base_address,
            pci.base_address,
            pci.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    info!(
        "physical memory: [{:#x}: {:#x})",
        meta.physical_memory_offset,
        meta.physical_memory_offset + meta.physical_memory_size
    );

    gpt.map_region(
        meta.physical_memory_offset,
        meta.physical_memory_offset,
        meta.physical_memory_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER,
    )?;

    Ok(gpt)
}

#[cfg(target_arch = "aarch64")]
pub fn setup_gpm(dtb: usize, kernel_entry: usize) -> Result<GuestPageTable> {
    let mut gpt = GuestPageTable::new()?;
    let meta = MachineMeta::parse(dtb);
    /* 
    for virtio in meta.virtio.iter() {
        gpt.map_region(
            virtio.base_address,
            virtio.base_address,
            0x1000, 
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
        debug!("finish one virtio");
    }
    */
    // hard code for virtio_mmio
    gpt.map_region(
        0xa000000,
        0xa000000,
        0x4000,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
    )?;
    
    if let Some(pl011) = meta.pl011 {
        gpt.map_region(
            pl011.base_address,
            pl011.base_address,
            pl011.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }


    for intc in meta.intc.iter() {
        gpt.map_region(
            intc.base_address,
            intc.base_address,
            intc.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    if let Some(pcie) = meta.pcie {
        gpt.map_region(
            pcie.base_address,
            pcie.base_address,
            pcie.size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )?;
    }

    info!(
        "physical memory: [{:#x}: {:#x})",
        meta.physical_memory_offset,
        meta.physical_memory_offset + meta.physical_memory_size
    );
    
    gpt.map_region(
        meta.physical_memory_offset,
        meta.physical_memory_offset,
        meta.physical_memory_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER,
    )?;
    
    gpt.map_region(
        GUEST_KERNEL_BASE_VADDR,
        kernel_entry,
        meta.physical_memory_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE | MappingFlags::USER,
    )?;

    Ok(gpt)
}
