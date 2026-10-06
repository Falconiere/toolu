//! toolu core, `runtime` layer: what a command learns from its process and the
//! calling plugin (argv, the binary's path, the plugin manifest, the version-skew
//! rule of #411), and what a `toolu` verb receives and returns (#442).
//!
//! What every binary needs to run on a host, without a shell parser or TLS (#414):
//!
//! - `env`: the environment snapshot every resolver reads (only this crate reads `std::env`);
//! - `host`: host detection, the config, project, state and plugin roots, and the
//!   Codex plugin snapshot;
//! - `config`: the `toolu.config.json` loader with its fail-closed envelope, gate
//!   modes, thresholds, model tiers, docs-sync globs, the permissions write and the
//!   plugin settings files;
//! - `git`: the repository holding a directory, read from `.git` without spawning git (#415);
//! - `process`: bounded subprocesses in their own process group (only this module spawns);
//! - `startup`: stable-path publishing, bounded context output, Codex dependency
//!   warnings and `TOOLU_STARTUP_REPORT` records;
//! - `registry`: registry events, file names, the module manifest and the `Rule` trait;
//! - `json`: JSON text as TypeScript and jq print it; `cli_args`: jq number arguments.

pub mod atomic;
pub mod cli;
pub mod cli_args;
pub mod config;
pub mod env;
pub mod git;
pub mod host;
pub mod install;
pub mod invocation;
pub mod json;
pub mod manifest;
pub mod namespace;
pub mod process;
pub mod registry;
pub mod skew;
pub mod startup;
pub mod version;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "runtime";
