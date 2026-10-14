//! `toolu epic close` and `toolu epic gate`.

use std::path::{Path, PathBuf};

use clap::{ArgMatches, Command};
use serde_json::json;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

use super::{document, failure, flag, option, text, usage};
use crate::close::{close_epic, require_complete, summary_table};
use crate::gate::{GateOpts, MakeTracker, Settle, run_gate};
use crate::graph::read_graph;
use crate::tracker::{Gh, TrackerKind, detect_tracker, make_tracker};

pub(super) fn close_command() -> Command {
  Command::new("close")
    .about("Close a completed epic in its tracker with the merged-PR summary")
    .arg(option("graph", "FILE", "The epic's graph.json").required(true))
    .arg(flag("dry-run", "Print the summary and close nothing"))
}

pub(super) fn gate_command() -> Command {
  Command::new("gate")
    .about("Verify a sub-issue PR against the merge policy and optionally merge it")
    .arg(option("pr", "REF", "The pull request").required(true))
    .arg(option(
      "issue",
      "REF",
      "The sub-issue to close and tick once merged",
    ))
    .arg(option("epic", "REF", "The epic to tick"))
    .arg(flag(
      "merge",
      "Merge on a merge verdict, pinned to the verified head",
    ))
    .arg(flag("auto", "Arm auto-merge on a wait verdict"))
    .arg(
      option(
        "method",
        "METHOD",
        "squash, merge or rebase; default the repository's",
      )
      .value_parser(["squash", "merge", "rebase"]),
    )
    .arg(option(
      "state-dir",
      "DIR",
      "The epic state directory whose record gets the verdict",
    ))
    .arg(option("key", "KEY", "The issue key of that record"))
    .arg(flag("dry-run", "Assess only"))
}

/// Close the epic, or print its summary.
pub(super) fn close(matches: &ArgMatches, ctx: &Ctx, env: &Env) -> Outcome {
  let graph = match read_graph(Path::new(text(matches, "graph").unwrap_or(""))) {
    Ok(graph) => graph,
    Err(err) => return failure("toolu epic close", &err),
  };
  if let Err(err) = require_complete(&graph) {
    return failure("toolu epic close", &err);
  }
  let now = toolu_state::time::epoch_millis(std::time::SystemTime::now());
  let paths = crate::paths::Paths::from_env(env);
  let metrics = crate::metrics::closing(&paths, Path::new(&graph.state_dir), now);
  if matches.get_flag("dry-run") {
    let summary = summary_table(&graph, metrics.as_deref());
    return if ctx.json {
      document(&json!({"epic": graph.epic.reference, "summary": summary}))
    } else {
      Outcome::data(summary)
    };
  }
  let kind = match detect_tracker(&graph.epic.reference, graph.tracker.as_deref(), env) {
    Ok(kind) => kind,
    Err(err) => return usage("toolu epic close", &err),
  };
  let repo = graph.default_repo.clone().unwrap_or_default();
  match make_tracker(kind, &graph.epic.reference, &repo, env)
    .and_then(|tracker| close_epic(&graph, tracker.as_ref(), metrics.as_deref()))
  {
    Ok(result) => document(&result),
    Err(err) => failure("toolu epic close", &err),
  }
}

/// Run the merge gate.
pub(super) fn gate(matches: &ArgMatches, env: &Env) -> Outcome {
  let owned = |id: &str| text(matches, id).map(str::to_owned);
  let opts = GateOpts {
    pr: owned("pr").unwrap_or_default(),
    issue: owned("issue"),
    epic: owned("epic"),
    merge: matches.get_flag("merge"),
    auto: matches.get_flag("auto"),
    method: owned("method"),
    state_dir: owned("state-dir").map(PathBuf::from),
    key: owned("key"),
    dry_run: matches.get_flag("dry-run"),
  };
  if let Err(err) = crate::refs::parse_ref(&opts.pr, None) {
    return usage("toolu epic gate", &err);
  }
  let gh = match Gh::new(env) {
    Ok(gh) => gh,
    Err(err) => return failure("toolu epic gate", &err),
  };
  let make: &MakeTracker<'_> =
    &|kind: TrackerKind, reference: &str, repo: &str| make_tracker(kind, reference, repo, env);
  let at = Settle { env, gh: &gh, make };
  match run_gate(&opts, &at) {
    Ok(result) => {
      notify_merged(&opts, &result, env);
      // The document still says what merged; the unwritten record fails the run.
      let mut outcome = document(&result);
      if result.get("record_error").is_some() {
        outcome.exit = toolu_protocol::exit::Exit::Failure;
      }
      outcome
    }
    Err(err) => failure("toolu epic gate", &err),
  }
}

/// Tell a live engine at once that `--key`'s PR merged, so it cleans up
/// without waiting for its next PR check (which would see the merge anyway).
fn notify_merged(opts: &GateOpts, result: &serde_json::Value, env: &Env) {
  let merged = result.get("merged") == Some(&serde_json::Value::Bool(true));
  let Some(key) = opts.key.as_deref().filter(|_| merged && !opts.dry_run) else {
    return;
  };
  let paths = crate::paths::Paths::from_env(env);
  if crate::lock::live(&paths.lock()) && paths.socket().exists() {
    let request = json!({"op": "event", "key": key, "event": "merged"});
    let _sent = crate::client::exchange_retry(&paths, crate::PROTOCOL, &request);
  }
}

#[cfg(test)]
#[path = "tests/settle_test.rs"]
mod tests;
