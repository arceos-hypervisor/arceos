mod boot;

pub mod generic_timer;
#[cfg(not(platform_family = "aarch64-raspi"))]
pub mod psci;

#[cfg(feature = "irq")]
pub mod gic;

cfg_if::cfg_if! {
    if #[cfg(any(platform_family = "aarch64-bsta1000b", platform_family= "aarch64-rk3588j"))] {
        pub mod dw_apb_uart;
        // pub mod console {
        //     pub use super::dw_apb_uart::*;
        // }
    } else if #[cfg(any(platform_family = "aarch64-raspi", platform_family = "aarch64-qemu-virt"))] {
        pub mod pl011;
        // pub mod console {
        //     pub use super::pl011::*;
        // }
    }
}
