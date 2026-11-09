//! Deterministic layered procedural generation, independent of rendering.
//!
//! Ported from LayerProCGen's C++ baseline 8fb913c. The browser adapter lives
//! in the separate layerprocgen-web crate and uses WebGL directly.

pub mod common;
pub mod core;
pub mod cultivation;
pub mod executor;
pub mod floating_origin;
pub mod geogrid;
pub mod landscape;
pub mod location;
pub mod lod;
pub mod path;
pub mod samples;
pub mod save_state;

/// Version of this Rust port.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
