//! The rust-quality rule crate: the post-edit Rust rules compiled into `toolu`,
//! the `SessionStart` hook that clears their stale gate state, and the
//! `toolu rust-quality` namespace with its `check` verb. `crates/cli` reaches it through the hub
//! (`crates/toolu`).

mod ast_rules;
mod ast_yaml;
mod check;
mod docs;
mod file;
mod layout_rules;
mod line_rules;
mod rule;
mod size_rules;

use clap::{ArgMatches, Command};
use serde_json::json;
use toolu_engine::quality::QualityPlugin;
use toolu_runtime::cli::{Ctx, Outcome};

/// The rule the manifest enables.
pub use rule::{RUST_QUALITY, RUST_QUALITY_BATCH};

/// The plugin this crate belongs to: `plugins/rust-quality`.
pub const PLUGIN: &str = "rust-quality";

/// The registry spec of the rule.
pub const SPEC: &str = rule::SPEC;

/// Its `SessionStart` retirement for `.rs` edits: removes the rule manifest a
/// registered session left, and prunes the old `.js` module and `.sh` bridge.
pub const QUALITY_PLUGIN: QualityPlugin = QualityPlugin {
  plugin: PLUGIN,
  spec: SPEC,
  name: rule::NAME,
};

const ABOUT: &str = "Rust quality rules for reviews and the push gate";

const GUIDE: &str = "rust-quality no longer runs after every edit; its SessionStart hook retires \
   any earlier registration. toolu rust-quality check runs the blocking rules over a whole \
   repository (or the paths you name). Blocking findings (inline test modules, misplaced tests, \
   lint suppression, unsafe code, unwrap/expect/panic in src, mocks) fail the file's quality-gate \
   entry; a failing gate denies git push while gates.qualityGate blocks — commits are never \
   stopped. Oversized files, functions and impl blocks and missing or verbose docs advise and \
   never block.\n\nLimits come from lang.rust in toolu.config.json (maxFileLines, maxFnLines, \
   maxImplLines, noMocks); crates allowed to use unsafe are listed in \
   settings/rust-unsafe-exemptions.txt.";

/// The `toolu rust-quality` namespace.
pub fn command() -> Command {
  Command::new(PLUGIN)
    .about(ABOUT)
    .long_about(GUIDE)
    .subcommand(check::command())
}

/// `toolu rust-quality check`, or the guide without a verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  if let Some(("check", check)) = matches.subcommand() {
    return check::run(check, ctx);
  }
  if ctx.json {
    let doc = json!({ "namespace": PLUGIN, "about": ABOUT, "text": GUIDE });
    return Outcome::data(doc.to_string());
  }
  Outcome::data(GUIDE.to_owned())
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
