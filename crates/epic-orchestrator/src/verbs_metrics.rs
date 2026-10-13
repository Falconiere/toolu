//! The measuring verbs of `toolu epic` (#451): `metrics` reads the journal,
//! `baseline` derives the TypeScript-era figures from recorded state
//! directories. Neither needs a running engine.

use std::path::PathBuf;
use std::time::SystemTime;

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_state::time::epoch_millis;

use crate::metrics::{self, Query, baseline};
use crate::paths::Paths;
use crate::verbs::{failed, json_out, text};

/// The verbs, for the `toolu epic` tree.
pub(crate) fn commands() -> Vec<Command> {
  vec![metrics_command(), baseline_command()]
}

/// Run `name` when it is one of these verbs.
pub(crate) fn run(name: &str, matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Option<Outcome> {
  Some(match name {
    "metrics" => metrics_verb(matches, ctx, env),
    "baseline" => baseline_verb(matches),
    _ => return None,
  })
}

fn metrics_command() -> Command {
  Command::new("metrics")
    .about("Measure epic runs from the engine journal, beside the TypeScript baseline")
    .arg(
      Arg::new("epic")
        .long("epic")
        .value_name("ID")
        .help("Only this epic's records"),
    )
    .arg(
      Arg::new("since")
        .long("since")
        .value_name("DATE")
        .help("Only records from this date (YYYY-MM-DD) or ISO-8601 UTC time on"),
    )
}

fn baseline_command() -> Command {
  Command::new("baseline")
    .about("Derive the TypeScript-era baseline from recorded epic state directories")
    .arg(
      Arg::new("state")
        .long("state")
        .value_name("DIR")
        .required(true)
        .help("The state root with one directory per epic"),
    )
    .arg(
      Arg::new("until")
        .long("until")
        .value_name("TIME")
        .required(true)
        .help("Count issues launched and merged by this ISO-8601 UTC time"),
    )
    .arg(
      Arg::new("outlier")
        .long("outlier")
        .value_name("EPIC")
        .action(ArgAction::Append)
        .help("An epic reported apart from the totals; repeatable"),
    )
}

fn metrics_verb(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let since_ms = match text(matches, "since").map(metrics::parse_since).transpose() {
    Ok(since) => since,
    Err(err) => return Outcome::failed(Exit::Usage, format!("toolu epic metrics: {err}")),
  };
  let query = Query {
    epic: text(matches, "epic").map(str::to_owned),
    since_ms,
    now_ms: epoch_millis(SystemTime::now()),
  };
  let doc = match metrics::document(&Paths::from_env(env).journal_dir(), &query) {
    Ok(doc) => doc,
    Err(err) => return failed("toolu epic metrics", &err),
  };
  if ctx.json {
    return json_out(&doc);
  }
  Outcome::data(metrics::render(&doc, &metrics::budget_rows(&doc)))
}

fn baseline_verb(matches: &ArgMatches) -> Outcome {
  let input = baseline::Input {
    state: text(matches, "state")
      .map(PathBuf::from)
      .unwrap_or_default(),
    until: text(matches, "until").unwrap_or("").to_owned(),
    outliers: matches
      .get_many::<String>("outlier")
      .map(|values| values.cloned().collect())
      .unwrap_or_default(),
  };
  match baseline::derive(&input)
    .and_then(|doc| serde_json::to_string_pretty(&doc).map_err(|err| err.to_string()))
  {
    Ok(text) => Outcome::data(text),
    Err(err) => failed("toolu epic baseline", &err),
  }
}

#[cfg(test)]
#[path = "tests/verbs_metrics_test.rs"]
mod tests;
