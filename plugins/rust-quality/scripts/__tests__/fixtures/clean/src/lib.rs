//! Clean library root with no structural violations.
//!
//! # Public API
//! Re-exports the arithmetic surface.
//!
//! # Usage
//! `clean_fixture::add(1, 2)`.

mod arithmetic;

pub use arithmetic::add;
