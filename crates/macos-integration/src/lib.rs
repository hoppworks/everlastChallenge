//! Native macOS objects stay behind this crate's interfaces.
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(
    unsafe_code,
    reason = "Native FFI is confined to this crate; every unsafe operation requires a safety argument."
)]

pub mod clipboard;
pub mod delivery;
pub mod keyboard;
pub mod main_delivery;
pub mod paste;
pub mod power;
pub mod target;
pub mod ui_services;
