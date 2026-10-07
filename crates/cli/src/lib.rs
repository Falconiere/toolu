//! The `toolu` binary crate. The command tree lives in the binary; this library
//! exists so `cargo test --lib` can see the status-snapshot link.

#[cfg(test)]
#[path = "tests/snapshot_link_test.rs"]
mod tests;
