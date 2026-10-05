//! One source of numbers: `maxFnLines` equals clippy's `too-many-lines-threshold`,
//! and every host copy of `lang.rust` equals `.claude/toolu.config.json`'s.

use std::path::Path;

use super::{Context, Finding};
use crate::data::{CONFIG, Limits};

/// Host configs that must repeat `lang.rust` exactly when present.
const COPIES: &[&str] = &[".codex/toolu.config.json"];

/// Disagreeing copies of a limit.
pub(super) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let root = ctx.workspace.root.as_path();
  let limits = &ctx.limits;
  let mut found = Vec::new();
  let text = std::fs::read_to_string(root.join("clippy.toml"))
    .map_err(|err| format!("cannot read clippy.toml: {err}"))?;
  let clippy: toml::Table = text.parse().map_err(|err| format!("clippy.toml: {err}"))?;
  let threshold = clippy
    .get("too-many-lines-threshold")
    .and_then(toml::Value::as_integer);
  if threshold != i64::try_from(limits.function).ok() {
    found.push(Finding::new(
      "thresholds",
      "clippy.toml",
      1,
      format!(
        "too-many-lines-threshold is {threshold:?}, {CONFIG} maxFnLines is {}; change both together",
        limits.function
      ),
    ));
  }
  for copy in COPIES {
    if !root.join(copy).is_file() {
      continue;
    }
    let other = crate::data::limits_at(root, Path::new(copy))?;
    if !same(limits, &other) {
      found.push(Finding::new(
        "thresholds",
        copy,
        1,
        format!("lang.rust differs from {CONFIG}; change every copy together"),
      ));
    }
  }
  Ok(found)
}

fn same(a: &Limits, b: &Limits) -> bool {
  (a.file, a.function, a.impl_block) == (b.file, b.function, b.impl_block)
}

#[cfg(test)]
#[path = "tests/thresholds_test.rs"]
mod tests;
