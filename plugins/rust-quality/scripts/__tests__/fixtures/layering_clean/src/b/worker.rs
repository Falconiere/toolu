//! Worker for module b.
//!
//! # Public API
//! run — uses only a's re-exported PublicThing.
use crate::a::PublicThing;

pub fn run() -> PublicThing {
    PublicThing
}
