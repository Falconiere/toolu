//! The python-quality rule crate: the post-edit Python rules compiled into
//! `toolu`, the `SessionStart` hook that clears their stale gate state, and the
//! `toolu python-quality` guide. `crates/cli` reaches it through the hub (`crates/toolu`).

mod docs;
mod file;
mod layout_rules;
mod no_mocks;
mod rule;
mod size_rules;
mod suppression;

use clap::{ArgMatches, Command};
use toolu_engine::quality::QualityPlugin;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The rule the manifest enables.
pub use rule::{PYTHON_QUALITY, PYTHON_QUALITY_BATCH};

/// The plugin this crate belongs to: `plugins/python-quality`.
pub const PLUGIN: &str = "python-quality";

/// Its `SessionStart` retirement for `.py` edits: removes the rule manifest a
/// registered session left, and prunes the old `.js` module and `.sh` bridge.
pub const QUALITY_PLUGIN: QualityPlugin = QualityPlugin {
  plugin: PLUGIN,
  spec: rule::SPEC,
  name: rule::NAME,
};

const GUIDE: Guide = Guide {
  name: "python-quality",
  about: "Python quality rules for reviews and the push gate",
  text: "python-quality has no verbs and no longer runs after every edit; its SessionStart hook \
   retires any earlier registration. The same findings reach the project through its own linter \
   and tests: a failing quality command marks the gate, and a failing gate denies git push while \
   gates.qualityGate blocks — commits are never stopped. Oversized files and functions advise \
   and never block.\n\nLimits come from lang.python in toolu.config.json (maxFileLines, \
   maxFnLines, noMocks).",
};

/// The `toolu python-quality` namespace.
pub fn command() -> Command {
  GUIDE.command()
}

/// `toolu python-quality`: print the guide.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  GUIDE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
