//! The ts-quality rule crate: the post-edit TypeScript rules compiled into
//! `toolu`, their `SessionStart` registration, and the `toolu ts-quality`
//! guide. `crates/cli` reaches it through the hub (`crates/toolu`).

mod advisories;
mod ast_rules;
mod ast_yaml;
mod file;
mod hooks;
mod layout_rules;
mod line_rules;
mod register;
mod rule;
mod size_rules;
mod type_rules;

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The Codex warning when the toolu core is missing.
pub use hooks::check_toolu;
/// Publish the manifest and its bridge at `SessionStart`.
pub use register::register;
/// The rule the manifest enables.
pub use rule::TS_QUALITY;

/// The plugin this crate belongs to: `plugins/ts-quality`.
pub const PLUGIN: &str = "ts-quality";

/// The registry spec of the rule.
pub const SPEC: &str = rule::SPEC;

const GUIDE: Guide = Guide {
  name: "ts-quality",
  about: "TypeScript quality rules that run after each edit",
  text: "ts-quality has no verbs: its rules run after every edit of a .ts or .tsx file in a \
   TypeScript project with a package manager. Violations (type assertions, console.log, \
   suppression comments, oversized files and functions, swallowed errors, mocks in tests and \
   more) fail the file's quality-gate entry until it is fixed; duplication, JSDoc and \
   unhandled-await findings are advisories.\n\nLimits come from lang.ts in toolu.config.json \
   (maxFileLines, maxFnLines, noMocks). Run toolu doctor to see whether the rule is \
   registered and compiled in.",
};

/// The `toolu ts-quality` namespace.
pub fn command() -> Command {
  GUIDE.command()
}

/// `toolu ts-quality`: print the guide.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  GUIDE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
