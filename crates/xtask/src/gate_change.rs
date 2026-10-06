//! `cargo xtask check-gate-change`: a change to gate data (a limit, a ban, a
//! lint level, the layer rules, a lowered floor) ships alone, in a
//! `chore(gates):` PR. Registration data may ship with the code it registers.

mod additive;
mod classify;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::options::Options;
use crate::{Verdict, data, output};

/// The default base when `--base` is absent.
const DEFAULT_BASE: &str = "origin/main";

/// The title prefix a gate change carries.
const GATES_PREFIX: &str = "chore(gates):";

/// Compare the working tree with the merge base of `--base` and HEAD.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let base_ref = options.base.as_deref().unwrap_or(DEFAULT_BASE);
  let base = git(root, &["merge-base", base_ref, "HEAD"])?
    .trim()
    .to_owned();
  let rules = format!("{}/rules.json", data::DATA_DIR);
  if show(root, &base, Path::new(&rules))?.is_none() {
    output::say(&format!(
      "check-gate-change: no gate baseline at {base_ref} ({rules} is absent); nothing to compare"
    ));
    return Ok(Verdict::Clean);
  }
  let mut gate = Vec::new();
  let mut product = Vec::new();
  for path in changed(root, &base)? {
    let before = show(root, &base, &path)?;
    let after = std::fs::read_to_string(root.join(&path)).ok();
    let change = classify::classify(&path, before.as_deref(), after.as_deref(), root)?;
    gate.extend(change.gate);
    if change.product {
      product.push(path.to_string_lossy().replace('\\', "/"));
    }
  }
  let found = verdict(&gate, &product, options.title.as_deref());
  Ok(output::findings("check-gate-change", &found))
}

/// The findings for a diff that changes `gate` data and `product` paths.
pub(crate) fn verdict(gate: &[String], product: &[String], title: Option<&str>) -> Vec<String> {
  let mut found = Vec::new();
  if gate.is_empty() {
    return found;
  }
  if !product.is_empty() {
    found.push(format!(
      "gate data changed ({}) together with product code ({}): ship the gate change alone, \
       in a `{GATES_PREFIX} …` PR with the reason in its body",
      gate.join("; "),
      product.join(", ")
    ));
  }
  if let Some(title) = title
    && !title.starts_with(GATES_PREFIX)
  {
    found.push(format!(
      "gate data changed ({}) in a PR titled `{title}`: a gate change is titled `{GATES_PREFIX} …`",
      gate.join("; ")
    ));
  }
  found
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Result<String, String> {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()
    .map_err(|err| format!("cannot run git: {err}"))?;
  if !output.status.success() {
    return Err(format!(
      "git {} failed: {}",
      args.join(" "),
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `path` at revision `base`, or `None` when it does not exist there.
pub(crate) fn show(root: &Path, base: &str, path: &Path) -> Result<Option<String>, String> {
  let spec = format!("{base}:{}", path.to_string_lossy().replace('\\', "/"));
  let exists = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["cat-file", "-e", &spec])
    .output()
    .map_err(|err| format!("cannot run git: {err}"))?
    .status
    .success();
  if !exists {
    return Ok(None);
  }
  git(root, &["show", &spec]).map(Some)
}

/// Paths that differ between `base` and the working tree, untracked files included.
fn changed(root: &Path, base: &str) -> Result<Vec<PathBuf>, String> {
  let diff = git(root, &["diff", "--name-only", "--no-renames", base])?;
  let untracked = git(root, &["ls-files", "--others", "--exclude-standard"])?;
  let mut paths: Vec<PathBuf> = diff
    .lines()
    .chain(untracked.lines())
    .map(PathBuf::from)
    .collect();
  paths.sort();
  paths.dedup();
  Ok(paths)
}

#[cfg(test)]
#[path = "tests/gate_change_test.rs"]
mod tests;
