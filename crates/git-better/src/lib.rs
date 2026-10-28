//! The git-better plugin's crate: semantic branches and commits that agents can
//! read. Its `semantic-git` registry rule checks every Bash/Shell command that
//! commits or names a new branch: a Conventional Commit subject with a scope, an
//! `Intent:` trailer, and `<type>/[<ref>-]<slug>` branch names. `toolu
//! git-better check` runs the same checks before the command does.

use clap::ArgMatches;
use toolu_runtime::cli::{Ctx, Outcome};

mod areas;
mod branch;
mod check;
mod cli;
mod format;
mod gitio;
mod note;
mod post;
mod pre;
mod register;
mod report;
mod settings;
mod setup;

/// The `commit-note` rule enabled by its manifest.
pub use post::COMMIT_NOTE;
/// The `semantic-git` rule enabled by its manifest.
pub use pre::SEMANTIC_GIT;
/// Publish this plugin's rule manifests at `SessionStart`.
pub use register::register;

/// The plugin this crate belongs to: `plugins/git-better`.
pub const PLUGIN: &str = "git-better";
/// The registry spec of this plugin's rules.
pub(crate) const SPEC: &str = "git-better@toolu";

/// The `toolu git-better` namespace.
pub fn command() -> clap::Command {
  cli::command()
}

/// Run a `toolu git-better` verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  cli::run(matches, ctx)
}

/// Whether a `toolu git-better` verb reads stdin.
pub fn needs_stdin(matches: &ArgMatches) -> bool {
  cli::needs_stdin(matches)
}
