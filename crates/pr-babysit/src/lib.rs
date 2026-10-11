//! The pr-babysit plugin's crate: the `toolu babysit` namespace, which keeps a
//! pull request moving until CI, its review threads and the review-bot verdict
//! are clear, and the plugin's `SessionStart` hooks.

use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgMatches, Command};
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickReport, TickRequest};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::invocation::{current_dir, current_exe};

mod cli;
mod collect;
mod dispatch;
mod error;
mod fixer;
#[cfg(test)]
mod fixture;
mod hooks;
mod json;
mod link;
mod lock;
mod pattern;
mod record;
#[cfg(test)]
mod recorded;
mod reduce;
mod route;
mod schedule;
mod sha1;
mod state;
mod tick;
mod verbs;
mod verdict;
mod wait;
mod writes;

pub use hooks::{check_binary, check_toolu};
pub use link::GitHubTick;

/// The plugin this crate belongs to: `plugins/pr-babysit`.
pub const PLUGIN: &str = "pr-babysit";

/// The pause between resolving a thread and confirming it.
const RESOLVE_PAUSE: Duration = Duration::from_secs(1);

/// The `toolu babysit` namespace.
pub fn command() -> Command {
  cli::command()
}

/// Run a `toolu babysit` verb with the process environment. Only
/// `parse-verdict` reads stdin; `needs_stdin` tells the caller when.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let env = Env::process();
  let host = ctx.host.unwrap_or_else(|| detect(&env, None).host);
  let toolu = current_exe().unwrap_or_else(|| PathBuf::from("toolu"));
  let cwd = current_dir().ok();
  let world = verbs::World {
    env: &env,
    host,
    github: toolu_github::Config::one_shot(&env),
    jev: toolu_jev_client::Config::default(),
    stdin: ctx.stdin.as_deref(),
    toolu: &toolu,
    cwd: cwd.as_deref(),
    pause: RESOLVE_PAUSE,
  };
  verbs::execute(matches, &world)
}

/// Whether the verb in `matches` reads stdin: `parse-verdict`.
pub fn needs_stdin(matches: &ArgMatches) -> bool {
  matches.subcommand_name() == Some("parse-verdict")
}

/// pr-babysit's [`BabysitTick`], which `crates/cli` hands the epic engine:
/// one scheduled check of the pull request with the process environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tick;

impl BabysitTick for Tick {
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError> {
    let config = toolu_github::Config::scheduled();
    let source = tick::Source::GitHub {
      config,
      page_size: 100,
    };
    link::tick(request, source, &Env::process())
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
