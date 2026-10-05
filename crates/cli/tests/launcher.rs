//! The generated `hooks.json` launcher (#412) driving the real `toolu` binary:
//! real `sh -c`, a reduced `PATH`, temporary homes and plugin roots, the real
//! Bun and the real npm wrapper. Shell stand-ins only for the signal deaths.

#[path = "helpers/sandbox.rs"]
mod sandbox;

#[path = "helpers/crash.rs"]
mod crash;
#[path = "helpers/fallback.rs"]
mod fallback;
#[path = "helpers/missing.rs"]
mod missing;
#[path = "helpers/native.rs"]
mod native;
#[path = "helpers/resolution.rs"]
mod resolution;
#[path = "helpers/skew.rs"]
mod skew;
