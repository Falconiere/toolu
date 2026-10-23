//! Map `owner/repo` to a local primary checkout (`scripts/checkouts.ts`): the
//! repositories herdr workspaces live in, their siblings and `EPIC_REPO_ROOTS`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

use crate::herdr::{Call, Client};
use crate::herdr_calls::request;
use crate::paths::Paths;

/// The lowercase `owner/repo` of `path`'s GitHub `origin`, or `None`.
pub(crate) fn origin_of(path: &Path) -> Option<String> {
  let mut spec = Spec::new([
    "git".to_owned(),
    "-C".to_owned(),
    path.display().to_string(),
    "remote".to_owned(),
    "get-url".to_owned(),
    "origin".to_owned(),
  ]);
  spec.timeout = Duration::from_secs(10);
  let output = run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.timed_out)?;
  let remote = Regex::new(r"github\.com[:/]([^/]+)/(.+?)(?:\.git)?/?$").ok()?;
  let caps = remote.captures(output.stdout.trim())?;
  Some(format!("{}/{}", caps.get(1)?.as_str(), caps.get(2)?.as_str()).to_lowercase())
}

/// The primary (not linked) repository roots of herdr's workspaces; none when
/// herdr cannot answer.
pub(crate) fn herdr_roots(paths: &Paths, env: &Env) -> Vec<PathBuf> {
  let Ok(reply) = request(&mut Client::new(), paths, env, &Call::WorkspaceList) else {
    return Vec::new();
  };
  reply
    .get("workspaces")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|workspace| {
      let tree = workspace.get("worktree")?;
      let linked = tree.get("is_linked_worktree").and_then(Value::as_bool) == Some(true);
      let root = tree.get("repo_root").and_then(Value::as_str)?;
      (!linked).then(|| PathBuf::from(root))
    })
    .collect()
}

/// Each repository's checkout among `known` roots, their parents' children
/// named like the repository, and `extra` parent directories.
pub(crate) fn resolve_checkouts(
  repos: &[String],
  known: &[PathBuf],
  extra: &[PathBuf],
) -> BTreeMap<String, Option<String>> {
  let wanted: BTreeMap<String, &String> = repos
    .iter()
    .map(|repo| (repo.to_lowercase(), repo))
    .collect();
  let mut found: BTreeMap<String, Option<String>> =
    repos.iter().map(|repo| (repo.clone(), None)).collect();
  let mut candidates = unique(known.to_vec());
  let parents = unique(
    known
      .iter()
      .filter_map(|root| root.parent().map(Path::to_path_buf))
      .chain(extra.iter().cloned())
      .collect(),
  );
  let names: BTreeSet<&str> = repos
    .iter()
    .filter_map(|repo| repo.split_once('/').map(|(_, name)| name))
    .collect();
  for parent in &parents {
    candidates.extend(names.iter().map(|name| parent.join(name)));
  }
  for path in candidates {
    if !path.join(".git").exists() {
      continue;
    }
    let Some(origin) = origin_of(&path) else {
      continue;
    };
    if let Some(canonical) = wanted.get(&origin)
      && let Some(slot) = found.get_mut(*canonical)
      && slot.is_none()
    {
      *slot = Some(path.display().to_string());
    }
  }
  found
}

/// `EPIC_REPO_ROOTS`, split on `:`.
pub(crate) fn repo_roots(env: &Env) -> Vec<PathBuf> {
  env
    .get("EPIC_REPO_ROOTS")
    .map(|value| {
      value
        .split(':')
        .filter(|part| !part.is_empty())
        .map(PathBuf::from)
        .collect()
    })
    .unwrap_or_default()
}

/// Where missing repositories are cloned: the parent of the epic repository's
/// checkout, else an `EPIC_REPO_ROOTS` entry, the parent of a herdr root, or
/// the parent of any other checkout. Never the working directory.
///
/// # Errors
/// Nothing known gives a directory other than `/`.
pub(crate) fn resolve_clone_root(
  epic_checkout: Option<String>,
  repo_roots: &[PathBuf],
  herdr_roots: &[PathBuf],
  checkouts: &BTreeMap<String, Option<String>>,
) -> Result<PathBuf, String> {
  let parent = |path: &Path| path.parent().map(Path::to_path_buf);
  let from_checkout = epic_checkout.iter().filter_map(|c| parent(Path::new(c)));
  let from_roots = repo_roots.iter().cloned();
  let from_herdr = herdr_roots.iter().filter_map(|root| parent(root));
  let from_others = checkouts
    .values()
    .flatten()
    .filter_map(|c| parent(Path::new(c)));
  from_checkout
    .chain(from_roots)
    .chain(from_herdr)
    .chain(from_others)
    .find(|dir| dir.parent().is_some() && dir.is_absolute())
    .ok_or_else(|| {
      "cannot resolve clone_root: set EPIC_REPO_ROOTS or open the repository in herdr".to_owned()
    })
}

fn unique(paths: Vec<PathBuf>) -> Vec<PathBuf> {
  let mut seen = BTreeSet::new();
  paths
    .into_iter()
    .filter(|path| seen.insert(path.clone()))
    .collect()
}

#[cfg(test)]
#[path = "tests/checkouts_test.rs"]
mod tests;
