//! Optional collaboration domain. No network, native turn controls, UI, or user configuration.
//! Callers and native turn observations must come from a trusted host adapter, never tool arguments.
mod model;
pub mod queue;
mod store;
pub use model::*;
pub use store::Store;
