//! Rule 11: the folder allowlist and Rust file naming and placement.

use std::collections::BTreeSet;
use std::path::Path;

use super::{Context, Finding};
use crate::workspace::parts;

/// Entries outside the allowlist, misnamed or misplaced Rust files.
pub(super) fn check(ctx: &Context<'_>) -> Vec<Finding> {
  let mut found: BTreeSet<Finding> = BTreeSet::new();
  for file in &ctx.workspace.files {
    if let Some(finding) = allowlist(ctx, file) {
      found.insert(finding);
    }
  }
  for source in &ctx.sources {
    found.extend(rust_file(ctx, &source.rel));
  }
  found.into_iter().collect()
}

fn entry(rule_list: &[String], dir: &str, name: &str) -> Option<Finding> {
  if rule_list.iter().any(|allowed| allowed == name) {
    return None;
  }
  let shown = if dir.is_empty() {
    name.to_owned()
  } else {
    format!("{dir}/{name}")
  };
  Some(Finding::new(
    "structure",
    &shown,
    1,
    format!(
      "`{name}` is not in the folder allowlist for `{}`",
      if dir.is_empty() { "." } else { dir }
    ),
  ))
}

/// The first allowlist `file` breaks, if any.
fn allowlist(ctx: &Context<'_>, file: &Path) -> Option<Finding> {
  let names = parts(file);
  let structure = &ctx.rules.structure;
  let first = names.first()?;
  if let Some(finding) = entry(&ctx.folders.root, "", first) {
    return Some(finding);
  }
  match names.as_slice() {
    [crates, core, name, _, ..] if crates == "crates" && core == "core" => {
      entry(&ctx.folders.core, "crates/core", name)
    }
    [crates, name, _, ..] if crates == "crates" => entry(&ctx.folders.crates, "crates", name),
    [plugins, name, inner, _, ..] if plugins == "plugins" => {
      entry(&structure.plugin, &format!("plugins/{name}"), inner)
    }
    _ => None,
  }
  .or_else(|| member_entry(ctx, file))
}

/// Entries directly inside a member directory and inside its `fuzz/` package.
fn member_entry(ctx: &Context<'_>, file: &Path) -> Option<Finding> {
  let member = ctx.workspace.member_of(file)?;
  let inside = parts(file.strip_prefix(&member.dir).ok()?);
  let dir = member.dir.to_string_lossy().replace('\\', "/");
  let structure = &ctx.rules.structure;
  match inside.as_slice() {
    [fuzz, name, ..] if fuzz == "fuzz" => entry(&structure.fuzz, &format!("{dir}/fuzz"), name),
    [name, ..] => entry(&structure.krate, &dir, name),
    [] => None,
  }
}

fn snake_case(stem: &str) -> bool {
  let mut chars = stem.chars();
  chars.next().is_some_and(|c| c.is_ascii_lowercase())
    && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Naming and placement of one Rust file.
fn rust_file(ctx: &Context<'_>, rel: &Path) -> Vec<Finding> {
  let path = rel.to_string_lossy().replace('\\', "/");
  let stem = rel
    .file_stem()
    .map(|stem| stem.to_string_lossy().into_owned())
    .unwrap_or_default();
  let structure = &ctx.rules.structure;
  let member = ctx.workspace.member_of(rel);
  let mut found = Vec::new();
  let mut flag = |message: String| found.push(Finding::new("structure", &path, 1, message));
  if !snake_case(&stem) {
    flag(format!("`{stem}.rs` is not a snake_case file name"));
  }
  if stem == "mod" {
    flag("a mod file — name the module file after the module".to_owned());
  }
  if stem == "build" {
    flag("a build script — the workspace has none".to_owned());
  }
  let crate_dir = member
    .and_then(|member| member.dir.file_name())
    .map(|name| name.to_string_lossy());
  if stem == "main"
    && !crate_dir.is_some_and(|name| structure.main_crates.iter().any(|main| *main == name))
  {
    flag(format!(
      "main.rs outside {}",
      structure.main_crates.join(", ")
    ));
  }
  if let Some(src) = member.and_then(|member| rel.strip_prefix(member.dir.join("src")).ok()) {
    let depth = src.components().count().saturating_sub(1);
    if depth > structure.max_src_depth {
      flag(format!(
        "{depth} directory levels under src, limit {}",
        structure.max_src_depth
      ));
    }
  }
  found
}

#[cfg(test)]
#[path = "tests/structure_test.rs"]
mod tests;
