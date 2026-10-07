//! Browser-free performance contracts. Hardware adapters are feature-gated.
pub mod audio;
pub mod bench;
pub mod checkpoint;
pub mod control;
#[cfg(feature = "controller")]
pub mod controller;
#[cfg(feature = "controller")]
pub mod controller_runtime;
pub mod ipc;
#[cfg(feature = "renderer")]
pub mod native;
pub mod notify;
pub mod shader;
pub mod show;
pub mod surface;
pub mod texture;
