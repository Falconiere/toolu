//! Module a.
//!
//! # Public API
//! Re-exports the public surface from the api submodule.
mod api;
mod internals;
pub use self::api::PublicThing;
