//! Fixed public reader package: core/runtime and optional HTTP service.
pub mod core;
pub mod runtime;
pub use crate::core::reader;
#[cfg(feature = "web")]
pub mod web;
