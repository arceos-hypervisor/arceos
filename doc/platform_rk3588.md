# How to run ArceOS on rk3588

Use Command `make ARCH=aarch64 PLATFORM=aarch64-rk3588j A=(pwd)/examples/helloworld kernel` to build the kernel image.
Then use the flash tool to write the generated `boot.img` to the rk3588 platform, and Arceos will be able to boot.


