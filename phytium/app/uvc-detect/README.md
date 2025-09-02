# 测试方法

1. 参照 `phytium/app/test-uvc/.project.toml-example` 修改 `.project.toml`

2. `ostool run uboot | tee target/uvc.log`

3. 待程序执行出以下字样后，ctrl+c 退出

    ```text
    000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
    FRAME_DATA_END
    [ 59.607662 0 phytium_test_uvc:147] First frame captured successfully, stopping stream...
    u-boot : get pfdi : 0 , gd_base = 0x30c3f000
    u-boot : send cmd to cpld : 12 
    u-boot: gpio power off
    ```

4. `cargo install --git  https://github.com/drivercraft/CrabUSB/ uvc-frame-parser`

5. `RUST_LOG=debug uvc-frame-parser --log-file target/uvc.log --output-dir target/output`

6. 查看 `target/output` 目录下的 `frame_*.jpg` 文件
