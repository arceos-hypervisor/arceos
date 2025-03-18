use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    axlog::ax_println!("[{}]:{}", axhal::cpu::this_cpu_id(), info);
    axhal::misc::terminate()
}
