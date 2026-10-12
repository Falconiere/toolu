//! The engine-action verbs of `toolu epic` (#435): `graph`, `route`,
//! `launch`, `finish`, `close`, `release`, `gate`, `jira` and `probe`.
//! The engine runs the graph build, routing, `launch`, `finish` and `close`
//! in process too; `release` asks a live engine to clear a hold (or edits the
//! record when none runs); `gate`, `jira` and `probe` are CLI only.

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

mod graph;
mod launch;
mod read;
mod settle;
mod teardown;

/// The verbs, for the `toolu epic` tree.
pub(crate) fn commands() -> Vec<Command> {
  vec![
    graph::graph_command(),
    graph::route_command(),
    launch::launch_command(),
    teardown::finish_command(),
    settle::close_command(),
    teardown::release_command(),
    settle::gate_command(),
    read::jira_command(),
    read::probe_command(),
  ]
}

/// Run `name` when it is one of these verbs.
pub(crate) fn run(name: &str, matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Option<Outcome> {
  Some(match name {
    "graph" => graph::graph(matches, ctx, env),
    "route" => graph::route(matches, ctx, env),
    "launch" => launch::launch(matches, ctx, env),
    "finish" => teardown::finish(matches, env),
    "close" => settle::close(matches, ctx, env),
    "release" => teardown::release(matches, env),
    "gate" => settle::gate(matches, env),
    "jira" => read::jira(matches, env),
    "probe" => read::probe(matches),
    _ => return None,
  })
}

/// A boolean flag.
fn flag(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name)
    .long(name)
    .action(ArgAction::SetTrue)
    .help(help)
}

/// A string option.
fn option(name: &'static str, value: &'static str, help: &'static str) -> Arg {
  Arg::new(name).long(name).value_name(value).help(help)
}

/// One string argument.
fn text<'a>(matches: &'a ArgMatches, id: &str) -> Option<&'a str> {
  matches.get_one::<String>(id).map(String::as_str)
}

/// A usage failure (exit 64).
fn usage(prefix: &str, err: &str) -> Outcome {
  Outcome::failed(Exit::Usage, format!("{prefix}: {err}"))
}

/// A failure (exit 1).
fn failure(prefix: &str, err: &str) -> Outcome {
  Outcome::failed(Exit::Failure, format!("{prefix}: {err}"))
}

/// One JSON document, pretty.
fn document(value: &serde_json::Value) -> Outcome {
  Outcome::data(serde_json::to_string_pretty(value).unwrap_or_default())
}

#[cfg(test)]
#[path = "tests/verbs_actions_test.rs"]
mod tests;
