//! The epic-orchestrator plugin's crate: the resident `toolu epic` engine (#434).
//! Graph, route, launch and the merge queue stay on `planned` (#435, #448).

use clap::{ArgMatches, Command};
use toolu_engine::babysit::BabysitTick;
use toolu_runtime::cli::{Ctx, Outcome};

mod attention;
mod attention_ops;
/// Cross-plugin babysit integration for the resident engine.
pub mod babysit;
mod checkpoint;
mod client;
mod commit;
mod control;
mod dashboard;
mod disk;
mod dispatch;
mod effects;
mod github_budget;
mod github_engine;
mod github_probe;
mod herdr;
mod herdr_argv;
mod http;
mod http_access;
mod http_request;
mod http_response;
mod http_stream;
mod job;
mod journal;
mod limit;
mod lock;
mod logic;
mod model;
mod note;
mod paths;
mod query;
mod recover;
mod schedule;
mod server;
mod server_snapshot;
mod snapshot;
mod socket;
mod socket_deadline;
mod socket_herdr;
mod source;
mod source_apply;
#[cfg(test)]
mod source_fix;
mod status;
mod status_api;
mod status_epics;
mod status_events;
mod status_resources;
#[cfg(test)]
mod status_schema;
mod verbs;
mod waiting_jobs;
mod watch;

/// Control-socket greeting. A client on another number spools and sends `replace`.
pub(crate) const PROTOCOL: u64 = 1;

/// Idle wake. Waiters sleep until this or their own deadline.
pub(crate) const TICK: std::time::Duration = std::time::Duration::from_secs(30);

/// The plugin this crate belongs to: `plugins/epic-orchestrator`.
pub const PLUGIN: &str = "epic-orchestrator";

/// The `toolu epic` namespace.
pub fn command() -> Command {
  verbs::command()
}

/// Run a `toolu epic` verb. `crates/cli` passes pr-babysit's tick; `babysit::next`
/// turns a tick failure into attention (#433).
pub fn run(matches: &ArgMatches, ctx: &Ctx, tick: &dyn BabysitTick) -> Outcome {
  verbs::run(matches, ctx, tick)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/idle_test.rs"]
mod idle_tests;
