# Usage

```shell
sudo apt install libudev-dev
cargo install ostool
ostool run uboot
```

```text
select target:
  0: aarch64-unknown-none
  1: aarch64-unknown-none-softfloat
  2: armebv7r-none-eabi
```

chose `1`.

```text
请选择构建系统：
  0: Cargo
  1: Custom
```

chose `1`.

```text
请输入构建命令：
make  A=phytium/app/helloworld LOG=debug LD_SCRIPT=link.x FEATURES=driver-dyn MYPLAT=axplat-aarch64-dyn
```

```text
请输入elf文件名（空为不需要，用于debug）：
```

put `enter` to skip.

```text
请输入kernel文件名：
phytium/app/helloworld/helloworld_aarch64-dyn.bin
```

```text
请选择串口设备
  0: /dev/ttyUSB0
  1: /dev/ttyS1
  2: /dev/ttyS10
```

chose your usb to serial device.

```text
请设置波特率:
115200
```

```text
请选择网卡
  0: [enp3s0] - [10.3.10.9]
  1: [lo] - [127.0.0.1]
  2: 无网络，用串口传输
```

chose 2.

```text
请输入dtb文件路径:
phytium/phytium.dtb
```

```text
内核：kernel.bin
DTB file not provided
启动命令：dcache flush;go $loadaddr
内核大小：0xe040
串口：/dev/ttyUSB0, 115200
等待 U-Boot 启动...

wait for `<INTERRUPT>`
```

put board power on or reset.
