//! The code-review plugin's crate: `toolu code-review run`, a native review of the
//! push diff (epic #660). `contract` is the JSON report agents read, `memory` the
//! records carried between review rounds, and `budget` every byte and token cap with
//! the effort levels. Each pipeline stage owns one module (`shape`, `rules`,
//! `prepass`, `review`, `validate`, `report`); `model` is the structured call's
//! output contract that `review` consumes. `run` sends nothing without the
//! repository's opt-in (`not-enabled`).

use clap::ArgMatches;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

/// Byte and token caps, and the effort levels.
pub mod budget;
mod cli;
/// The JSON report contract.
pub mod contract;
/// The expectation-based eval harness: schema, matcher and report.
pub mod eval;
/// Review-memory records.
pub mod memory;
/// Strict model findings before grounding and anchoring.
pub mod model;
mod pipeline;

/// Deterministic scanners that run before the model.
pub mod prepass;
mod report;
/// The prompt, the review call and the Jev judge.
pub mod review;
/// Repository conventions from the base ref.
pub mod rules;
/// The `codeReview` config section: the opt-in and the effort preference.
pub mod settings;
/// The committed diff and its reviewable anchors.
pub mod shape;
/// Grounding and anchoring of findings in the shaped diff.
pub mod validate;

/// The plugin this crate belongs to: `plugins/code-review`.
pub const PLUGIN: &str = "code-review";

/// The `toolu code-review` namespace.
pub fn command() -> clap::Command {
  cli::command()
}

/// Run a `toolu code-review` verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some(("run", verb)) => match pipeline::run(&env(ctx), ctx, verb, &review::Options::default()) {
      Ok(report) => report::present(&report, ctx),
      Err(message) => Outcome::failed(Exit::Failure, format!("code-review: {message}")),
    },
    Some(("eval", verb)) => eval::cli::run(verb, ctx, &env(ctx)),
    Some(("dismiss", verb)) => dismiss(verb, ctx),
    _ => Outcome::failed(Exit::Usage, "code-review: a verb is required".to_owned()),
  }
}

/// The process environment, with `--config-dir` as `TOOLU_CONFIG_DIR`.
fn env(ctx: &Ctx) -> Env {
  let env = Env::process();
  match &ctx.config_dir {
    Some(dir) => env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy()),
    None => env,
  }
}

fn dismiss(verb: &ArgMatches, ctx: &Ctx) -> Outcome {
  let id = verb
    .get_one::<String>("finding-id")
    .map(String::as_str)
    .unwrap_or_default();
  let reason = verb
    .get_one::<String>("reason")
    .map(String::as_str)
    .unwrap_or_default();
  match pipeline::dismiss(&env(ctx), ctx, id, reason) {
    // `--json` promises one JSON document on stdout.
    Ok(message) if ctx.json => {
      Outcome::data(serde_json::json!({"id": id, "message": message}).to_string())
    }
    Ok(message) => Outcome::data(message),
    Err(message) => Outcome::failed(Exit::Failure, format!("code-review: {message}")),
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
