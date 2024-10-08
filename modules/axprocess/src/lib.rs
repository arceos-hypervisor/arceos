#![cfg_attr(not(test), no_std)]
mod api;
pub use api::*;
mod process;
pub use process::{Process, PID2PC, TID2TASK};

#[macro_use]
extern crate log;

extern crate alloc;

pub mod flags;
pub mod futex;
pub mod link;
pub mod loader;
mod stdio;

mod fd_manager;

mod syscall;

#[cfg(feature = "hv")]
mod scf;
// #[cfg(feature = "signal")]
// pub mod signal;
