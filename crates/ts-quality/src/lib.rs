//! The ts-quality rule crate: the post-edit TypeScript rules compiled into
//! `toolu`, the `SessionStart` hook that clears their stale gate state, and the
//! `toolu ts-quality` guide. `crates/cli` reaches it through the hub (`crates/toolu`).

mod advisories;
mod ast_rules;
mod ast_yaml;
mod file;
mod layout_rules;
mod line_rules;
mod rule;
mod size_rules;
mod type_rules;

use clap::{ArgMatches, Command};
use toolu_engine::quality::QualityPlugin;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The rule the manifest enables.
pub use rule::{TS_QUALITY, TS_QUALITY_BATCH};

/// The plugin this crate belongs to: `plugins/ts-quality`.
pub const PLUGIN: &str = "ts-quality";

/// The registry spec of the rule.
pub const SPEC: &str = rule::SPEC;

/// Its `SessionStart` retirement for `.ts`/`.tsx` edits: removes the rule
/// manifest a registered session left, and prunes the old `.js` module and
/// `.sh` bridge.
pub const QUALITY_PLUGIN: QualityPlugin = QualityPlugin {
  plugin: PLUGIN,
  spec: SPEC,
  name: rule::NAME,
};

const GUIDE: Guide = Guide {
  name: "ts-quality",
  about: "TypeScript quality rules for reviews and the push gate",
  text: "ts-quality has no verbs and no longer runs after every edit; its SessionStart hook \
   retires any earlier registration. The same findings reach the project through its own lint \
   and tests: a failing quality command marks the gate, and a failing gate denies git push \
   while gates.qualityGate blocks — commits are never stopped. Oversized files and functions \
   advise and never block.\n\nLimits come from lang.ts in toolu.config.json (maxFileLines, \
   maxFnLines, noMocks).",
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
