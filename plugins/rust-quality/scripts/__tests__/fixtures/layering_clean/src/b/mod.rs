//! Module b.
//!
//! # Public API
//! Consumes module a's public surface.
mod worker;
pub use self::worker::run;
