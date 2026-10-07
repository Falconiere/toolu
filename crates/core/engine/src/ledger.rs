//! The delivery-flow plan ledger (`packages/toolu-core/src/ledger`, #421):
//! `toolu ledger run`, `status`, `preflight`, `path`, `root` and `self-test`.
//! For `version: 1` files it makes the same decisions and writes the same
//! bytes and messages as the TypeScript CLI (`plugins/toolu/hooks/dist/plan-ledger.js`),
//! so either implementation continues a ledger the other wrote.
//!
//! - `jq`: the jq semantics the ledger, verdict and waiver code reproduce;
//! - `parse`: the plan's steps block, header fields and the spec's AC ids;
//! - `model`, `entries` and `coverage`: recompute, step entries and the AC report;
//! - `io`: reading, writing and locating the ledger.

/// The AC-coverage report.
pub mod coverage;
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
