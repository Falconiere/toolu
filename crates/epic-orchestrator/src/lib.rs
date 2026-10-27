//! The epic-orchestrator plugin's crate: the resident `toolu epic` engine (#434),
//! its graph, route and launch actions (#435) and its merge queue (#448).

use clap::{ArgMatches, Command};
use toolu_engine::babysit::BabysitTick;
use toolu_runtime::cli::{Ctx, Outcome};

mod attention;
mod attention_ops;
/// Cross-plugin babysit integration for the resident engine.
pub mod babysit;
mod babysit_next;
mod checkpoint;
mod client;
mod close;
mod commit;
mod commit_queue;
mod control;
mod coupling;
mod dashboard;
mod disarm;
mod disk;
mod dispatch;
mod dispatch_ops;
mod effect_thread;
mod effects;
mod effects_cleanup;
mod effects_epic;
mod effects_launch;
mod effects_queue;
mod effects_run;
mod engine_usage;
mod epic_close;
mod epics;
mod executor;
mod finish;
#[cfg(test)]
#[path = "tests/finish_rig_test.rs"]
mod finish_rig;
mod fleet;
mod fleet_cli;
#[cfg(test)]
#[path = "tests/fleet_rig_test.rs"]
mod fleet_rig;
mod gate;
#[cfg(test)]
#[path = "tests/gate_rig_test.rs"]
mod gate_rig;
mod github_budget;
mod github_engine;
mod github_probe;
mod graph;
mod herdr;
#[cfg(test)]
#[path = "tests/herdr_agents_test.rs"]
mod herdr_agents;
mod herdr_argv;
mod herdr_calls;
#[cfg(test)]
#[path = "tests/herdr_fixture_test.rs"]
mod herdr_fixture;
mod herdr_interrupt;
#[cfg(test)]
#[path = "tests/herdr_state_test.rs"]
mod herdr_state;
mod holds;
/// Native hook commands for the epic plugin.
pub mod hook;
mod hosts;
mod http;
mod http_access;
mod http_fleet;
mod http_metrics;
mod http_request;
mod http_response;
mod http_stream;
mod jira_cli;
mod job;
mod journal;
mod launch;
#[cfg(test)]
#[path = "tests/launch_rig_test.rs"]
mod launch_rig;
#[cfg(test)]
#[path = "tests/leak_test.rs"]
mod leak;
mod limit;
mod lock;
mod logic;
mod merge_bridge;
mod metrics;
mod model;
mod note;
mod notify;
mod notify_notice;
mod paths;
#[cfg(test)]
#[path = "tests/peer_fixture_test.rs"]
mod peer_fixture;
#[cfg(test)]
#[path = "tests/plan_rig_test.rs"]
mod plan_rig;
mod probe_evidence;
mod query;
mod queue;
mod queue_fields;
#[cfg(test)]
#[path = "tests/queue_rig_test.rs"]
mod queue_rig;
#[cfg(test)]
#[path = "tests/queue_scenario_test.rs"]
mod queue_scenario;
mod queue_step;
mod reach;
mod rebase;
mod recover;
mod refill;
#[cfg(test)]
#[path = "tests/refill_rig_test.rs"]
mod refill_rig;
mod refresh;
mod refs;
mod retry;
mod route;
#[cfg(test)]
#[path = "tests/scenario_test.rs"]
mod scenario;
#[cfg(test)]
#[path = "tests/scenario_flow_test.rs"]
mod scenario_flow;
#[cfg(test)]
#[path = "tests/scenario_rig_test.rs"]
mod scenario_rig;
mod schedule;
mod server;
mod server_snapshot;
mod service;
mod snapshot;
mod socket;
mod socket_deadline;
mod socket_herdr;
mod socket_loop;
mod source;
mod source_apply;
#[cfg(test)]
mod source_fix;
mod status;
mod status_api;
mod status_epics;
mod status_events;
mod status_plan;
mod status_resources;
#[cfg(test)]
mod status_schema;
#[cfg(test)]
#[path = "tests/support_test.rs"]
mod support;
mod tracker;
mod verbs;
mod verbs_actions;
mod verbs_metrics;
mod verbs_notify;
mod wait_journal;
mod wait_ops;
mod waiting_jobs;
mod watch;
mod workload;

/// Control-socket greeting. A client on another number spools and sends `replace`.
pub(crate) const PROTOCOL: u64 = 2;

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
