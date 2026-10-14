//! The rust-quality rule crate: the post-edit Rust rules compiled into `toolu`,
//! their `SessionStart` registration, and the `toolu rust-quality` namespace
//! with its `check` verb. `crates/cli` reaches it through the hub
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
pub use rule::RUST_QUALITY;

/// The plugin this crate belongs to: `plugins/rust-quality`.
pub const PLUGIN: &str = "rust-quality";

/// The registry spec of the rule.
pub const SPEC: &str = rule::SPEC;

/// Its `SessionStart` registration and the bridge for `.rs` edits.
pub const QUALITY_PLUGIN: QualityPlugin = QualityPlugin {
  plugin: PLUGIN,
  spec: SPEC,
  name: rule::NAME,
};

const ABOUT: &str = "Rust quality rules that run after each edit";

const GUIDE: &str = "rust-quality's rules run after every edit of a .rs file in a Cargo project \
   with cargo on PATH. Violations (oversized files, functions and impl blocks, inline test \
   modules, misplaced tests, lint suppression, unsafe code, unwrap/expect/panic in src, \
   mocks) fail the file's quality-gate entry until it is fixed; missing or verbose docs are \
   advisories.\n\nLimits come from lang.rust in toolu.config.json (maxFileLines, maxFnLines, \
   maxImplLines, noMocks); crates allowed to use unsafe are listed in \
   settings/rust-unsafe-exemptions.txt. toolu rust-quality check runs the same blocking rules \
   over a whole repository. Run toolu doctor to see whether the rule is registered and \
   compiled in.";

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
