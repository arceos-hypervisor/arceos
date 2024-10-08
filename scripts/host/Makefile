QEMU ?= qemu-system-x86_64
PORT ?= 2333
TELNET_PORT ?= 4321
WORKSPACE ?= $(shell pwd)/../../..
target_bin := $(WORKSPACE)/arceos/apps/hv/hv_pc-x86.bin
jailhouse := $(WORKSPACE)/jailhouse-arceos
guest := $(WORKSPACE)/arceos/scripts/guest/*

OUT_ELF ?= ../../apps/hv/hv_pc-x86.elf

qemu_image := ubuntu-20.04-server-cloudimg-amd64.img
qemu_args := \
  -smp 4 -m 8G -accel kvm -nographic \
  -machine q35,kernel_irqchip=split \
  -cpu host,-kvm-asyncpf,-kvm-pv-eoi,-kvm-pv-ipi,-kvm-pv-sched-yield,-kvm-pv-unhalt,-kvm-steal-time,-kvmclock \
  -drive file=$(qemu_image) \
  -net user,id=net,hostfwd=tcp::$(PORT)-:22 -net nic,model=e1000e \
  -D qemu.log -d in_asm,int,mmu,pcall,cpu_reset,guest_errors \
  -serial mon:stdio \
  -serial telnet:localhost:$(TELNET_PORT),server,nowait

$(qemu_image):
	wget https://cloud-images.ubuntu.com/releases/focal/release/$(qemu_image)

.ONESHELL:
image: $(qemu_image)
	cat >user-data <<EOF
	#cloud-config
	password: guest
	chpasswd: { expire: False }
	ssh_pwauth: True
	EOF
	cloud-localds user-data.img user-data
	qemu-img resize $(qemu_image) +64G
	$(QEMU) $(qemu_args) -drive file=user-data.img,format=raw

echo_workspace:
	echo ${WORKSPACE}

qemu: $(qemu_image)
	$(QEMU) $(qemu_args)

qemu_debug: $(qemu_image)
	$(QEMU) $(qemu_args) -s -S

qemu_gdb:
	gdb-multiarch $(OUT_ELF) \
	  -ex 'target remote localhost:1234' \
	  -ex 'continue' \
	  -ex 'disp /16i $$pc'

# You can configure a ssh authorization through:
# 	cat ~/.ssh/id_rsa.pub| ssh -p 2333 ubuntu@localhost "cat - >> ~/.ssh/authorized_keys"

ssh:
	ssh -p $(PORT) ubuntu@localhost

scp_bin:
	scp -P $(PORT) -r $(target_bin) ubuntu@localhost:/home/ubuntu/arceos-intel.bin

scp_jailhouse:
	scp -P $(PORT) -r $(jailhouse) ubuntu@localhost:/home/ubuntu

scp_guest:
	scp -P $(PORT) -r $(guest) ubuntu@localhost:/home/ubuntu
