pub mod device_emu;

extern crate alloc;
use bit_field::BitField;
use alloc::{sync::Arc, vec, vec::Vec};
use spin::Mutex;
use core::marker::PhantomData;
use libax::hv::{Result as HyperResult, VmExitInfo, VCpu, HyperCraftHal, PerCpuDevices, PerVmDevices, VmxExitReason};
use libax::hv::{Error as HyperError, VmExitInfo as VmxExitInfo, HyperCraftHalImpl};

use device_emu::{VirtMsrDevice, PortIoDevice, Bundle, VirtLocalApic, ApicBaseMsrHandler};

use vm_config::{DeviceType, EmuDeviceType, PCIDevice, PassthroughDeviceType, VmConfigEntry};
use device_emu::pci_dev::PCI_DEVICES;

const VM_EXIT_INSTR_LEN_RDMSR: u8 = 2;
const VM_EXIT_INSTR_LEN_WRMSR: u8 = 2;
const VM_EXIT_INSTR_LEN_VMCALL: u8 = 3;

pub struct DeviceList<H: HyperCraftHal> {
    port_io_devices: Vec<Arc<Mutex<dyn PortIoDevice>>>,
    msr_devices: Vec<Arc<Mutex<dyn VirtMsrDevice>>>,
    marker: core::marker::PhantomData<H>,
}

impl<H: HyperCraftHal> DeviceList<H> {
    pub fn new() -> Self {
        Self { port_io_devices: vec![], msr_devices: vec![], marker: core::marker::PhantomData }
    }

    pub fn add_port_io_device(&mut self, device: Arc<Mutex<dyn PortIoDevice>>) {
        self.port_io_devices.push(device)
    }

    pub fn add_port_io_devices(&mut self, devices: &mut Vec<Arc<Mutex<dyn PortIoDevice>>>) {
        self.port_io_devices.append(devices)
    }

    pub fn find_port_io_device(&self, port: u16) -> Option<&Arc<Mutex<dyn PortIoDevice>>> {
        self.port_io_devices
            .iter()
            .find(|dev| dev.lock().port_range().contains(&port))
    }

    pub fn add_msr_device(&mut self, device: Arc<Mutex<dyn VirtMsrDevice>>) {
        self.msr_devices.push(device)
    }

    pub fn add_msr_devices(&mut self, devices: &mut Vec<Arc<Mutex<dyn VirtMsrDevice>>>) {
        self.msr_devices.append(devices)
    }

    pub fn find_msr_device(&self, msr: u32) -> Option<&Arc<Mutex<dyn VirtMsrDevice>>> {
        self.msr_devices
            .iter()
            .find(|dev| dev.lock().msr_range().contains(&msr))
    }

    fn handle_io_instruction_to_device(vcpu: &mut VCpu<H>, exit_info: &VmxExitInfo, device: &Arc<Mutex<dyn PortIoDevice>>) -> HyperResult {
        let io_info = vcpu.io_exit_info().unwrap();
        trace!(
            "VM exit: I/O instruction @ {:#x}: {:#x?}",
            exit_info.guest_rip,
            io_info,
        );

        if io_info.is_string {
            error!("INS/OUTS instructions are not supported!");
            return Err(HyperError::NotSupported);
        }
        if io_info.is_repeat {
            error!("REP prefixed I/O instructions are not supported!");
            return Err(HyperError::NotSupported);
        }
        if io_info.is_in {
            let value = device.lock().read(io_info.port, io_info.access_size)?;
            let rax = &mut vcpu.regs_mut().rax;
            // SDM Vol. 1, Section 3.4.1.1:
            // * 32-bit operands generate a 32-bit result, zero-extended to a 64-bit result in the
            //   destination general-purpose register.
            // * 8-bit and 16-bit operands generate an 8-bit or 16-bit result. The upper 56 bits or
            //   48 bits (respectively) of the destination general-purpose register are not modified
            //   by the operation.
            match io_info.access_size {
                1 => *rax = (*rax & !0xff) | (value & 0xff) as u64,
                2 => *rax = (*rax & !0xffff) | (value & 0xffff) as u64,
                4 => *rax = value as u64,
                _ => unreachable!(),
            }
        } else {
            let rax = vcpu.regs().rax;
            let value = match io_info.access_size {
                1 => rax & 0xff,
                2 => rax & 0xffff,
                4 => rax,
                _ => unreachable!(),
            } as u32;
            device.lock().write(io_info.port, io_info.access_size, value)?;
        }
        vcpu.advance_rip(exit_info.exit_instruction_length as _)?;
        Ok(())
    }

    pub fn handle_io_instruction(&mut self, vcpu: &mut VCpu<H>, exit_info: &VmxExitInfo) -> Option<HyperResult> {
        let io_info = vcpu.io_exit_info().unwrap();
        
        if let Some(dev) = self.find_port_io_device(io_info.port) {
            return Some(Self::handle_io_instruction_to_device(vcpu, exit_info, dev));
        } else {
            return None;
            // panic!(
            //     "Unsupported I/O port {:#x} access: {:#x?}\n, vcpu: {:#x?}",
            //     io_info.port, io_info, vcpu
            // )
        }
    }

    pub fn handle_msr_read(&mut self, vcpu: &mut VCpu<H>) -> HyperResult {
        let msr = vcpu.regs().rcx as u32;
    
        if let Some(dev) = self.find_msr_device(msr) {
            match dev.lock().read(msr) {
                Ok(value) => {
                    trace!("VM exit: RDMSR({:#x}) -> {:#x}", msr, value);

                    vcpu.regs_mut().rax = value & 0xffff_ffff;
                    vcpu.regs_mut().rdx = value >> 32;

                    vcpu.advance_rip(VM_EXIT_INSTR_LEN_RDMSR)?;
                    Ok(())
                },
                Err(e) => {
                    panic!("Failed to handle RDMSR({:#x}): {:?}", msr, e);
                },
            }
        } else {
            panic!("Unsupported RDMSR {:#x}, vcpu: {:#x?}", msr, vcpu);
        }
    }

    pub fn handle_msr_write(&mut self, vcpu: &mut VCpu<H>) -> HyperResult {
        let msr = vcpu.regs().rcx as u32;
        let value = (vcpu.regs().rax & 0xffff_ffff) | (vcpu.regs().rdx << 32);
    
        if let Some(dev) = self.find_msr_device(msr) {
            match dev.lock().write(msr, value) {
                Ok(_) => {   
                    trace!("VM exit: WRMSR({:#x}) <- {:#x}", msr, value);
                    
                    vcpu.advance_rip(VM_EXIT_INSTR_LEN_WRMSR)?;
                    Ok(())
                },
                Err(e) => {
                    panic!("Failed to handle WRMSR({:#x}): {:?}", msr, e);
                },
            }
        } else {
            panic!("Unsupported WRMSR {:#x}, vcpu: {:#x?}", msr, vcpu);
        }
    }    
}

pub struct X64VcpuDevices<H: HyperCraftHal> {
    pub(crate) devices: DeviceList<H>,
    pub(crate) apic_timer: Arc<Mutex<VirtLocalApic>>,
    pub(crate) bundle: Arc<Mutex<Bundle>>,
    pub(crate) console: Arc<Mutex<device_emu::Uart16550<device_emu::MultiplexConsoleBackend>>>,
    // pub(crate) pic: [Arc<Mutex<device_emu::I8259Pic>>; 2],
    pub(crate) pic: Vec<Arc<Mutex<device_emu::I8259Pic>>>,
    last: Option<u64>,
    marker: PhantomData<H>,
}

impl<H: HyperCraftHal> PerCpuDevices<H> for X64VcpuDevices<H> {
    fn new(vcpu: &VCpu<H>, config: VmConfigEntry) -> HyperResult<Self> {
        
        let mut devices = DeviceList::new();
        let mut pic: Vec<Arc<Mutex<device_emu::I8259Pic>>> = Vec::new(); 

        // emu device initialize from hard code
        // 1: bundle
        let bundle = Arc::new(Mutex::new(Bundle::new()));
        devices.add_port_io_device(Arc::new(Mutex::new(Bundle::proxy_system_control_a(&bundle))));
        devices.add_port_io_device(Arc::new(Mutex::new(Bundle::proxy_system_control_b(&bundle))));
        devices.add_port_io_device(Arc::new(Mutex::new(Bundle::proxy_cmos(&bundle))));
        devices.add_port_io_device(Arc::new(Mutex::new(Bundle::proxy_pit(&bundle))));
        // 2: apic
        let apic_timer = Arc::new(Mutex::new(VirtLocalApic::new()));
        devices.add_msr_device(Arc::new(Mutex::new(VirtLocalApic::msr_proxy(&apic_timer))));
        devices.add_msr_device(Arc::new(Mutex::new(ApicBaseMsrHandler{})));
        // emu device initialize from config
        let emu_dev_config = &config.vm_emu_dev_config_list;
        for emu_dev in &emu_dev_config.emu_dev_list {
            match emu_dev.emu_type {
                EmuDeviceType::EmuDevicePIC => {
                    let port_cnt = emu_dev.base.len();
                    for i in 0..port_cnt {
                        let tmp_pic = Arc::new(Mutex::new(device_emu::I8259Pic::new(emu_dev.base[i] as _, emu_dev.range[i] as _)));
                        pic.push(tmp_pic.clone());
                        devices.add_port_io_device(tmp_pic);
                    }
                },
                EmuDeviceType::EmuDevicePCI => {
                    let pci_config_space = device_emu::PCIConfigurationSpace::new(emu_dev.base[0] as _, emu_dev.range[0] as _);
                    // let pci_config_space = device_emu::PCIPassthrough::new(emu_dev.base[0] as _);
                    devices.add_port_io_device(Arc::new(Mutex::new(pci_config_space)));
                },
                EmuDeviceType::EmuDeviceDebugPort => {
                    let debug_port = device_emu::DebugPort::new(emu_dev.base[0] as _, emu_dev.range[0] as _);
                    devices.add_port_io_device(Arc::new(Mutex::new(debug_port)));
                },
                EmuDeviceType::EmuDeviceUart16550 => {
                    // COM2-COM4
                    let port_cnt = emu_dev.base.len();
                    for i in 0..port_cnt {
                        devices.add_port_io_device(Arc::new(Mutex::new(<device_emu::Uart16550>::new(emu_dev.base[i] as _, emu_dev.range[i] as _))));
                    }
                },
                EmuDeviceType::EmuDevicePit => {
                    warn!("need to adjust the factory define for pit");
                },
                EmuDeviceType::EmuDeviceCmos => {
                    warn!("need to adjust the factory define for cmos");
                },
                EmuDeviceType::EmuDeviceDummy => {
                    match emu_dev.device_type {
                        DeviceType::Pio => {
                            let dummy_device = device_emu::Dummy::new(emu_dev.base[0] as _, emu_dev.range[0] as _);
                            devices.add_port_io_device(Arc::new(Mutex::new(dummy_device)));
                        },
                        DeviceType::Msr => {
                            let dummy_device = device_emu::MsrDummy::new(emu_dev.base[0] as _, emu_dev.range[0] as _);
                            devices.add_msr_device(Arc::new(Mutex::new(dummy_device)));
                        }
                        _ => {
                            warn!("Unsupported dummy device type: {:?}", emu_dev.device_type);
                        }
                    }
                },
                EmuDeviceType::EmuDeviceVirtialLocalApic => {
                    warn!("need to adjust the factory define for VirtialLocalApic");
                },
                EmuDeviceType::EmuDeviceApic => {
                    warn!("need to adjust the factory define for Apic");
                },
                _ => {
                    warn!("Unsupported emu device type: {:?}", emu_dev.emu_type);
                }
            }
        }
        // passthrough device initialize from config
        let passthrough_dev_config = &config.vm_passthrough_dev_config_list;
        for passthrough_dev in &passthrough_dev_config.passthrough_dev_list {
            match passthrough_dev.device_type {
                PassthroughDeviceType::PCI => {
                    let pci_device = passthrough_dev.pci.clone().unwrap();
                    let bdf: u32  =  (pci_device.bus<<16) | (pci_device.slot<<11) | (pci_device.func<<8);
                    /* 
                    let pci_dev = PCIDevice {
                        vendor_id: pci_info.vendor_id,
                        device_id: pci_info.device_id,
                        command: pci_info.command,
                        status: pci_info.status,
                        revision_id_class_code: pci_info.revision_id_class_code,
                        cacheline_size: pci_info.cacheline_size,
                        latency_timer: pci_info.latency_timer,
                        header_type: pci_info.header_type,
                        bist: pci_info.bist,
                        bar: pci_info.bar,
                        cardbus_cis_pointer: pci_info.cardbus_cis_pointer,
                        subsystem_vendor_id: pci_info.subsystem_vendor_id,
                        subsystem_id: pci_info.subsystem_id,
                        expansion_rom_base_address: pci_info.expansion_rom_base_address,
                        capabilities_pointer: pci_info.capabilities_pointer,
                        _reserved1: 0,
                        _reserved2: 0,
                        interrupt_line: pci_info.interrupt_line,
                        interrupt_pin: pci_info.interrupt_pin,
                        min_gnt: pci_info.min_gnt,
                        max_lat: pci_info.max_lat,
                        capabilities: pci_info.capabilities,

                        bar_size: pci_info.bar_size,
                        expansion_rom_base_address_size: pci_info.expansion_rom_base_address_size,
                        msix_address: pci_info.msix_address,
                        msix_region_size: pci_info.msix_region_size,
                        num_msix_vectors: pci_info.num_msix_vectors,

                    };*/
                    let mut pci_devices = PCI_DEVICES.write();
                    pci_devices.insert(bdf, pci_device);
                },
                PassthroughDeviceType::PORT => {
                    let port_info = passthrough_dev.port.clone().unwrap();
                    let passthrough_port_device = device_emu::PortPassthrough::new(port_info.base[0] as _, port_info.range[0] as _);
                    devices.add_port_io_device(Arc::new(Mutex::new(passthrough_port_device)));
                },
                PassthroughDeviceType::MMIO => {
                    warn!("passthrough mmio device todo");
                }
                _ => {
                    warn!("Unsupported passthrough device type: {:?}", passthrough_dev.device_type);
                }
            }
        }
        // hard code for console device
        let console = Arc::new(Mutex::new(device_emu::Uart16550::<device_emu::MultiplexConsoleBackend>::new(0x3f8, 0x8)));
        // return the devices struct
        Ok(Self { 
            apic_timer,
            bundle,
            console,
            devices,
            pic,
            last: None,
            marker: PhantomData,
        })
/* 
        let mut apic_timer = Arc::new(Mutex::new(VirtLocalApic::new()));
        let mut bundle = Arc::new(Mutex::new(Bundle::new()));
        let mut console = Arc::new(Mutex::new(device_emu::Uart16550::<device_emu::MultiplexConsoleBackend>::new(0x3f8, 8)));
        let mut pic: [Arc<Mutex<device_emu::I8259Pic>>; 2]  = [
            Arc::new(Mutex::new(device_emu::I8259Pic::new(0x20, 2))),
            Arc::new(Mutex::new(device_emu::I8259Pic::new(0xA0, 2))),
        ];

        *console.lock().backend() = device_emu::MultiplexConsoleBackend::new_secondary(1, "sleep\n");

        let mut devices = DeviceList::new();

        let mut pmio_devices: Vec<Arc<Mutex<dyn PortIoDevice>>> = vec![
            // console.clone(), // COM1
            Arc::new(Mutex::new(<device_emu::PortPassthrough>::new(0x3f8, 8))),
            Arc::new(Mutex::new(<device_emu::Uart16550>::new(0x2f8, 8))), // COM2
            Arc::new(Mutex::new(<device_emu::Uart16550>::new(0x3e8, 8))), // COM3
            Arc::new(Mutex::new(<device_emu::Uart16550>::new(0x2e8, 8))), // COM4
            pic[0].clone(), // PIC1
            pic[1].clone(), // PIC2
            Arc::new(Mutex::new(device_emu::DebugPort::new(0x80, 1))), // Debug Port
            /*
                the complexity:
                - port 0x70 and 0x71 is for CMOS, but bit 7 of 0x70 is for NMI
                - port 0x40 ~ 0x43 is for PIT, but port 0x61 is also related
             */
            Arc::new(Mutex::new(Bundle::proxy_system_control_a(&bundle))),
            Arc::new(Mutex::new(Bundle::proxy_system_control_b(&bundle))),
            Arc::new(Mutex::new(Bundle::proxy_cmos(&bundle))),
            Arc::new(Mutex::new(Bundle::proxy_pit(&bundle))),
            Arc::new(Mutex::new(device_emu::Dummy::new(0xf0, 2))), // 0xf0 and 0xf1 are ports about fpu
            Arc::new(Mutex::new(device_emu::Dummy::new(0x3d4, 2))), // 0x3d4 and 0x3d5 are ports about vga
            Arc::new(Mutex::new(device_emu::Dummy::new(0x87, 1))), // 0x87 is a port about dma
            Arc::new(Mutex::new(device_emu::Dummy::new(0x60, 1))), // 0x60 and 0x64 are ports about ps/2 controller
            Arc::new(Mutex::new(device_emu::Dummy::new(0x64, 1))), // 
            // Arc::new(Mutex::new(device_emu::PCIConfigurationSpace::new(0xcf8, 8))),
            Arc::new(Mutex::new(device_emu::PCIPassthrough::new(0xcf8))),
        ];

        devices.add_port_io_devices(&mut pmio_devices);
        devices.add_msr_device(Arc::new(Mutex::new(VirtLocalApic::msr_proxy(&apic_timer))));
        devices.add_msr_device(Arc::new(Mutex::new(ApicBaseMsrHandler{})));
        // linux read this amd-related msr on my intel cpu for some unknown reason... make it happy
        devices.add_msr_device(Arc::new(Mutex::new(device_emu::MsrDummy::new(0xc0011029, 1))));

        Ok(Self { 
            apic_timer,
            bundle,
            console,
            devices,
            pic: pic.to_vec(),
            last: None,
            marker: PhantomData,
        })*/
      
    }

    fn vmexit_handler(&mut self, vcpu: &mut VCpu<H>, exit_info: &VmExitInfo) -> Option<HyperResult> {
        match exit_info.exit_reason {
            VmxExitReason::IO_INSTRUCTION => self.devices.handle_io_instruction(vcpu, exit_info),
            VmxExitReason::MSR_READ => Some(self.devices.handle_msr_read(vcpu)),
            VmxExitReason::MSR_WRITE => Some(self.devices.handle_msr_write(vcpu)),
            _ => None,
        }
    }

    fn hypercall_handler(&mut self, vcpu: &mut VCpu<H>, id: u32, args: (u32, u32)) -> HyperResult<u32> {
        Err(HyperError::NotSupported)
    }

    fn check_events(&mut self, vcpu: &mut VCpu<H>) -> HyperResult {
        if self.apic_timer.lock().inner.check_interrupt() {
            vcpu.queue_event(self.apic_timer.lock().inner.vector(), None);
        }

        // it's naive but it works.
        // inject 0x30(irq 0) every 1 ms after 10 seconds after booting.
        match self.last {
            Some(last) => {
                let now = libax::time::current_time_nanos();
                if now > 1_000_000 + last {
                    if !self.pic[0].lock().mask().get_bit(0) {
                        vcpu.queue_event(0x30, None);
                        let mask = self.pic[0].lock().mask();
                        // debug!("0x30 queued, mask {mask:#x}");
                    }
                    self.last = Some(now);
                }
            },
            None => {
                self.last = Some(libax::time::current_time_nanos() + 10_000_000_000);
            },
        }

        Ok(())
    }
}

pub struct X64VmDevices<H: HyperCraftHal> {
    devices: DeviceList<H>,
    marker: PhantomData<H>,
}

impl<H: HyperCraftHal> X64VmDevices<H> {
    fn handle_external_interrupt(vcpu: &VCpu<H>) -> HyperResult {
        let int_info = vcpu.interrupt_exit_info()?;
        trace!("VM-exit: external interrupt: {:#x?}", int_info);

        if int_info.vector != 0xf0 {
            panic!("VM-exit: external interrupt: {:#x?}", int_info);
        }

        assert!(int_info.valid);

        libax::hv::dispatch_host_irq(int_info.vector as usize)
    }
}

impl<H: HyperCraftHal> PerVmDevices<H> for X64VmDevices<H> {
    fn new() -> HyperResult<Self> {
        let devices = DeviceList::new();
        Ok(Self { marker: PhantomData, devices, })
    }

    fn vmexit_handler(&mut self, vcpu: &mut VCpu<H>, exit_info: &VmExitInfo) -> Option<HyperResult> {
        match exit_info.exit_reason {
            VmxExitReason::EXTERNAL_INTERRUPT => Some(Self::handle_external_interrupt(vcpu)),
            VmxExitReason::EPT_VIOLATION => {
                match vcpu.nested_page_fault_info() {
                    Ok(fault_info) => panic!(
                        "VM exit: EPT violation @ {:#x}, fault_paddr={:#x}, access_flags=({:?}), vcpu: {:#x?}",
                        exit_info.guest_rip, fault_info.fault_guest_paddr, fault_info.access_flags, vcpu
                    ),
                    Err(err) => panic!(
                        "VM exit: EPT violation with unknown fault info @ {:#x}, vcpu: {:#x?}",
                        exit_info.guest_rip, vcpu
                    ),
                }
            },
            VmxExitReason::IO_INSTRUCTION => self.devices.handle_io_instruction(vcpu, exit_info),
            VmxExitReason::MSR_READ => Some(self.devices.handle_msr_read(vcpu)),
            VmxExitReason::MSR_WRITE => Some(self.devices.handle_msr_write(vcpu)),
            _ => None,
        }
    }
}
