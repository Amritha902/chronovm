//! chronovm — a stack-based bytecode VM with a time-travel debugger.
//!
//! The core (`isa`, `assembler`, `vm`, `query`) is UI-agnostic and compiles
//! everywhere, including to `wasm32`. The terminal debugger (`tui`) is behind
//! the default `tui` feature; the browser bindings (`wasm`) are behind the
//! `wasm` feature. This split is what lets the exact same VM and causal engine
//! drive both the ratatui UI and the web UI.

pub mod assembler;
pub mod isa;
pub mod vm;

// Internal to the crate: used by both the terminal and browser UIs.
pub(crate) mod query;

#[cfg(feature = "tui")]
pub mod tui;

#[cfg(feature = "wasm")]
mod wasm;
