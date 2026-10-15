//! The python-quality rule crate: the post-edit Python rules compiled into
//! `toolu`, their `SessionStart` registration, and the `toolu python-quality`
//! guide. `crates/cli` reaches it through the hub (`crates/toolu`).

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
pub use rule::PYTHON_QUALITY;

/// The plugin this crate belongs to: `plugins/python-quality`.
pub const PLUGIN: &str = "python-quality";

/// Its `SessionStart` registration for `.py` edits: the manifest, and the prune of the old
/// `.js` module and `.sh` bridge.
pub const QUALITY_PLUGIN: QualityPlugin = QualityPlugin {
  plugin: PLUGIN,
  spec: rule::SPEC,
  name: rule::NAME,
};

const GUIDE: Guide = Guide {
  name: "python-quality",
  about: "Python quality rules that run after each edit",
  text: "python-quality has no verbs: its rules run after every edit of a .py file in a \
   Python project (pyproject.toml, setup.py, setup.cfg or requirements.txt) with python3 on \
   PATH. Violations (oversized files and functions, misplaced or misnamed tests, bare except, \
   blanket # noqa or # type: ignore, mocks in tests) fail the file's quality-gate entry until \
   it is fixed; a public def or class without a docstring is an advisory.\n\nLimits come from \
   lang.python in toolu.config.json (maxFileLines, maxFnLines, noMocks). Run toolu doctor to \
   see whether the rule is registered and compiled in.",
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
