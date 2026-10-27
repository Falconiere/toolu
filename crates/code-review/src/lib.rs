//! The code-review plugin's crate: `toolu code-review run`, a native review of the
//! push diff (epic #660). `contract` is the JSON report agents read, `memory` the
//! records carried between review rounds, and `budget` every byte and token cap with
//! the effort levels. Each pipeline stage owns one module (`shape`, `rules`,
//! `prepass`, `review`, `validate`, `report`); until they land, `run` fails closed as
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

mod prepass;
mod report;
mod review;
mod rules;
mod shape;
mod validate;

use contract::{ErrorKind, Report};

/// The plugin this crate belongs to: `plugins/code-review`.
pub const PLUGIN: &str = "code-review";

/// Why a run fails closed until the pipeline stages land.
const NOT_BUILT: &str =
  "the review pipeline is not built yet (#662-#669); nothing was sent and no state was written";

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
  if ctx.json {
    return match serde_json::to_string(report) {
      Ok(doc) => Outcome {
        exit: report.exit(),
        stdout: Some(doc),
        stderr: None,
      },
      Err(err) => Outcome::failed(Exit::Failure, format!("code-review: {err}")),
    };
  }
  match &report.error {
    Some(error) => Outcome::failed(
      report.exit(),
      format!("code-review: {}: {}", error.kind.name(), error.message),
    ),
    None => Outcome {
      exit: report.exit(),
      stdout: Some(format!(
        "code-review: {}, {} blocking finding(s)",
        report.verdict.name(),
        report.blocking_count()
      )),
      stderr: None,
    },
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
