mod boot;

pub mod generic_timer;
#[cfg(not(platform_family = "aarch64-raspi"))]
pub mod psci;

#[cfg(all(feature = "irq", feature = "gicv3"))]
pub mod gicv3;
#[cfg(all(feature = "irq", feature = "gicv3"))]
pub use gicv3 as gic;

#[cfg(all(feature = "irq", not(feature = "gicv3")))]
pub mod gicv2;
#[cfg(all(feature = "irq", not(feature = "gicv3")))]
pub use gicv2 as gic;

// #[cfg(not(feature = "virtio_console"))]
pub mod pl011;

// #[cfg(feature = "virtio_console")]
pub mod virtio_console;
