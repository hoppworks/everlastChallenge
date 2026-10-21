#![forbid(unsafe_code)]
//! Native application services. GUI entry and test/measurement tools share them.

pub mod audio;
pub mod contracts;
pub mod control;
pub mod controller;
pub mod desktop;
pub mod header;
mod hotkey_diagnostics;
pub mod hotkeys;
pub mod inference_service;
pub mod ipc;
pub mod model;
pub mod native_service;
pub(crate) mod pipeline;
pub mod preferences;
pub mod runtime;
pub mod settings;
pub mod setup;
pub mod startup_settings;
pub mod transcription;

pub mod application;
pub mod devices;

pub mod feedback;
pub mod hotkey_labels;
