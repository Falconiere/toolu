//! `cargo xtask check-cli-compat [--base REF] [--title TEXT]`: within a major
//! version every documented command, alias, flag, value and exit code of
//! `toolu` keeps working, because the binary and the plugins are upgraded
//! separately (#442, #411). It compares `docs/cli/commands.json` at the merge
//! base with the working tree, which the gate's `docs-cli` step has just proved
//! matches the binary. A break is allowed only when `hookProtocol` increased
//! and, given `--title`, the title carries the conventional `!` marker.

mod compare;

use std::path::Path;

use serde_json::Value;

use crate::gate_change::{git, show};
use crate::options::Options;
use crate::{Verdict, output};

/// The command tree `cargo xtask docs-cli` writes.
pub(crate) const TREE: &str = "docs/cli/commands.json";

/// The default base when `--base` is absent.
const DEFAULT_BASE: &str = "origin/main";

/// Compare the command tree at the merge base of `--base` and HEAD with the working tree.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let base_ref = options.base.as_deref().unwrap_or(DEFAULT_BASE);
  let base = git(root, &["merge-base", base_ref, "HEAD"])
    .map_err(|err| format!("{err} — pass --base <ref>, a revision this repository has"))?
    .trim()
    .to_owned();
  let Some(before) = show(root, &base, Path::new(TREE))? else {
    output::say(&format!(
      "check-cli-compat: no command tree at {base_ref} ({TREE} is absent); nothing to compare"
    ));
    return Ok(Verdict::Clean);
  };
  let after = std::fs::read_to_string(root.join(TREE))
    .map_err(|err| format!("cannot read {TREE}: {err} — run `cargo xtask docs-cli`"))?;
  let before = parse(&before, "the merge base")?;
  let after = parse(&after, "the working tree")?;
  let breaks = compare::breaks(&before, &after);
  let found = compare::verdict(
    &breaks,
    protocol(&before),
    protocol(&after),
    options.title.as_deref(),
  );
  if found.is_empty() && !breaks.is_empty() {
    output::say(&format!(
      "check-cli-compat: {} documented change(s) allowed by the hookProtocol bump",
      breaks.len()
    ));
  }
  Ok(output::findings("check-cli-compat", &found))
}

fn parse(text: &str, at: &str) -> Result<Value, String> {
  serde_json::from_str(text).map_err(|err| format!("{TREE} at {at} is not JSON: {err}"))
}

fn protocol(tree: &Value) -> u64 {
  tree
    .get("hookProtocol")
    .and_then(Value::as_u64)
    .unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/cli_compat_test.rs"]
mod tests;
