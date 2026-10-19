//! The generated `hooks.json` launcher (#412) driving the real `toolu` binary:
//! real `sh -c`, a reduced `PATH`, temporary homes and plugin roots, the real
//! npm wrapper, the Muse resolver (#546), and no Bun fallback. Shell stand-ins
//! only for the signal deaths.

#[path = "helpers/sandbox.rs"]
mod sandbox;

#[path = "helpers/crash.rs"]
mod crash;
#[path = "helpers/missing.rs"]
mod missing;
#[path = "helpers/muse.rs"]
mod muse;
#[path = "helpers/native.rs"]
mod native;
#[path = "helpers/no_bun.rs"]
mod no_bun;
#[path = "helpers/resolution.rs"]
mod resolution;
#[path = "helpers/skew.rs"]
mod skew;
