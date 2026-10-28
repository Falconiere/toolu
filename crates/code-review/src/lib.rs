//! The code-review plugin's crate: `toolu code-review run`, a native review of the
//! push diff (epic #660). `contract` is the JSON report agents read, `memory` the
//! records carried between review rounds, and `budget` every byte and token cap with
//! the effort levels. Each pipeline stage owns one module (`shape`, `rules`,
//! `prepass`, `review`, `validate`, `report`); `model` is the structured call's
//! output contract that `review` consumes. Until they land, `run` fails closed as
//! `unconfigured`, sends nothing and writes no state.

use clap::ArgMatches;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};

/// Byte and token caps, and the effort levels.
pub mod budget;
mod cli;
/// The JSON report contract.
pub mod contract;
/// Review-memory records.
pub mod memory;
/// Strict model findings before grounding and anchoring.
pub mod model;

/// Deterministic scanners that run before the model.
pub mod prepass;
mod report;
mod review;
/// Repository conventions from the base ref.
pub mod rules;
/// The `codeReview` config section: the opt-in and the effort preference.
pub mod settings;
/// The committed diff and its reviewable anchors.
pub mod shape;
mod validate;

use contract::{ErrorKind, Report};

/// The plugin this crate belongs to: `plugins/code-review`.
pub const PLUGIN: &str = "code-review";

/// Why a run fails closed until the pipeline stages land.
const NOT_BUILT: &str = "the review pipeline is not built yet (#662-#666, #668, #669); nothing was sent and no state was written";

/// The `toolu code-review` namespace.
pub fn command() -> clap::Command {
  cli::command()
}

/// Run a `toolu code-review` verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("run", verb)) => {
      let report = Report::failed(cli::effort(verb), ErrorKind::Unconfigured, NOT_BUILT);
      present(&report, ctx)
    }
    _ => Outcome::failed(Exit::Usage, "code-review: a verb is required".to_owned()),
  }
}

/// The report as one JSON document under `--json`, otherwise one line.
fn present(report: &Report, ctx: &Ctx) -> Outcome {
  let stdout = if ctx.json {
    match serde_json::to_string(report) {
      Ok(doc) => doc,
      Err(err) => return Outcome::failed(Exit::Failure, format!("code-review: {err}")),
    }
  } else if let Some(error) = &report.error {
    let line = format!("code-review: {}: {}", error.kind.name(), error.message);
    return Outcome::failed(report.exit(), line);
  } else {
    summary(report)
  };
  Outcome {
    exit: report.exit(),
    stdout: Some(stdout),
    stderr: None,
  }
}

/// The verdict line; unreviewed paths block too, so a blocked run names them.
fn summary(report: &Report) -> String {
  let unreviewed = match report.coverage.unreviewed.len() {
    0 => String::new(),
    count => format!(", {count} unreviewed path(s)"),
  };
  format!(
    "code-review: {}, {} blocking finding(s){unreviewed}",
    report.verdict().name(),
    report.blocking_count()
  )
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
