//! Platform-neutral core of NewTrans.
//!
//! Nothing in this crate touches the network, the clipboard, or OS input
//! APIs, so every rule here is covered by plain unit tests.

pub mod chain;
pub mod doubletap;
pub mod hotkey;
pub mod lang;
pub mod settings;
pub mod text;
