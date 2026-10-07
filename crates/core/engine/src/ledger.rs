//! The delivery-flow plan ledger (`packages/toolu-core/src/ledger`, #421):
//! `toolu ledger run`, `status`, `preflight`, `path`, `root` and `self-test`.
//! For `version: 1` files it makes the same decisions and writes the same
//! bytes and messages as the TypeScript CLI (`plugins/toolu/hooks/dist/plan-ledger.js`),
//! so either implementation continues a ledger the other wrote.
//!
//! - `jq`: the jq semantics the ledger, verdict and waiver code reproduce;
//! - `parse`: the plan's steps block, header fields and the spec's AC ids;
//! - `model`, `entries` and `coverage`: recompute, step entries and the AC report;
//! - `io`: reading, writing and locating the ledger;
//! - `check` and `scope`: a step's check run and its path-scoped hash;
//! - `context`, `run`, `commands` and `preflight`: the verbs.

/// A step's check run.
pub mod check;
/// `status`, `path`, `root` and `self-test`.
pub mod commands;
/// `run`'s flags and context.
pub mod context;
/// The AC-coverage report.
pub mod coverage;
/// Plan and spec header fields and acceptance-criterion ids.
pub mod doc;
/// Ledger step entries.
pub mod entries;
/// Ledger I/O and location.
pub mod io;
/// jq semantics over `Ordered` values.
pub mod jq;
/// Summary recompute, the summary line and orphan healing.
pub mod model;
/// Plan and spec parsing.
pub mod parse;
/// `preflight`.
pub mod preflight;
/// `run`.
pub mod run;
/// Per-step path scopes.
pub mod scope;
/// One step's run.
pub mod step;
