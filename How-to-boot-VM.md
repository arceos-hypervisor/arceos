# ArceOS-Hypervisor

This document aims to provide some useful commands for running ArceOS hypervisor as well as guest VM booting.

To boot ArceOS hypervisor from Linux, our modified jailhouse kernel module driver is necessary. 

When the following scripts being executed, the repo [jailhouse-arceos](https://github.com/arceos-hypervisor/jailhouse-arceos) is expected to be in the same folder as Arceos repository.

Otherwise, you will need to manually copy the folder `jailhouse-arceos` into guest Linux filesystem.

## Setup environment 

Boot Linux by QEMU. Scripts in `scripts/host` help to setup the guest Linux rootfs.

First, download a Linux image from [cloud-init](https://cloud-init.io/) and boot it by QEMU:

```bash
# in scripts/host
cd scripts/host
make image
```

The above command should be executed **exactly once**. And after setup, the following should be used instead:

```bash
# in scripts/host
make qemu
```

Note that after QEMU being started, **another terminal could be used to connect to it through telnet**. Two serial ports are provided for upper-level virtual machine: COM0 at 0x3f8 and COM1 at 0x2f8.

* COM0 connected to mon:std,
* COM1 bound to loopback interface, with TCP port `4321`.

```bash
telnet localhost 4321
```

See [script](scripts/host/Makefile) for details.

## Build ArceOS-HV

Build ArceOS-HV in its root directory, by either command:

```bash
make A=apps/hv HV=y TYPE1_5=y ARCH=x86_64 STRUCT=Hypervisor GUEST=nimbos LOG=debug SMP=2 build
# The following command copies the binary image file into Linux rootfs automatically.
make A=apps/hv HV=y TYPE1_5=y ARCH=x86_64 STRUCT=Hypervisor GUEST=nimbos LOG=debug SMP=2 scp_linux
```

## Copy scripts and image files

Files inside the `scripts/guest` are meant to be copied to Linux rootfs. Fortunately, scripts are prepared to do so:

```bash
# in scripts/host
./scp.sh
```

It works by invoking `make`s for three different targets. Refer to `scripts/host/Makefile` for specifics.

## Setup environment inside Linux rootfs

**The remaining steps need to be performed within the Linux environment that has just booted.**

You can login by SSH to the guest Linux with prepared scripts, and in this way, QEMU serial ports are no longer needed:

* From Host:

```bash
# in scripts/host. 
make ssh
```

* From Guest:

```bash
# in guest /home/ubuntu
./setup.sh
```

Guest will reboot, and after which, console of host Linux is changed to ttyS1, corresponding to COM1 at 0x2f8.

## Boot arceos-hypervisor

Before proceeding, take some time looking on the file `gen-config.sh` from [jailhouse-arceos](https://github.com/arceos-hypervisor/jailhouse-arceos) folder.

Reserved memory space size is set in this file for arceos-hypervisor, with default set to 4G:

```bash
# Line 2
sudo python3 ./tools/jailhouse-config-create --mem-hv 4G ./configs/x86/qemu-arceos.c
# ...
# Line 13
cmdline='memmap=0x100000000\\\\\\$0x100000000 console=ttyS1'
```
**Reduce the reserved memory in case your hardware doesn't support that much.**

Also note that reserved memory must be larger than physical memory size configured in [configuration file](modules/axconfig/src/platform/pc-x86-hv-type15.toml).

To boot arceos-hypervisor:

```bash
# guest /home/ubuntu
./enable-arceos-hv.sh
```

arceos-hypervisor will be booted and initialized, and return to linux. Upon this, the original guest Linux has downgraded to a guest VM running on the arceos-hypervisor.

It's also possible to start another guest through jailhouse cmd tool, like this:

```bash
# in guest /home/ubuntu, just for example, not meant to executed
sudo ${PATH_TO_JAILHOUSE_TOOL} axvm create CPU_MASK VM_TYPE BIOS_IMG KERNEL_IMG RAMDISK_IMG
```

Some scripts are ready for it:

```bash
# in guest /home/ubuntu
./boot_nimbios.sh
./boot_linux.sh
```

## Boot Guest VM

### [NimbOS](https://github.com/equation314/nimbos)
You can find its bios [here](apps/hv/guest/nimbos/bios).

### Linux
Currently, the vanilla Linux kernel is not supported (though I hope it will be).

This modified Linux kernel [linux-5.10.35](https://github.com/arceos-hypervisor/linux-5.10.35-rt/tree/tracing) with RT patch can run on arceos-hypervisor.

You need [vlbl](apps/hv/guest/vlbl) for bootloader, you can find vlbl.bin in its target dir.
You need to build your own ramdisk image, you can find helpful guides [here](https://github.com/OS-F-4/usr-intr/blob/main/ppt/%E5%B1%95%E7%A4%BA%E6%96%87%E6%A1%A3/linux-kernel.md#%E5%88%9B%E5%BB%BA%E6%96%87%E4%BB%B6%E7%B3%BB%E7%BB%9F%E4%BB%A5busybox%E4%B8%BA%E4%BE%8B).

## Emulated Devices

We are working on virtio devices...
