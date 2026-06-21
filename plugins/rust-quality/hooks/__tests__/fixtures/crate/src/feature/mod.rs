//! Feature module — VIOLATING mod.rs (contains logic).
//!
//! # Public API
//! Re-exports the feature's surface, but also carries a helper fn it shouldn't.

mod thing;

pub use thing::Thing;

fn helper() -> u32 {
    42
}
