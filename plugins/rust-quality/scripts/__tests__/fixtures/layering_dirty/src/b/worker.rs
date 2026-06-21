//! Worker for module b.
//!
//! # Public API
//! run — drives the worker.
use crate::a::internal_thing;

pub fn run() -> u32 {
    internal_thing()
}
