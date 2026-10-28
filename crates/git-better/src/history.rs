//! A range of commits read for an agent: each commit's subject parts and
//! trailers, its areas from `--numstat`, and the branch and host of its
//! provenance note. Two bounded `git log` calls, whatever the range's length.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use toolu_runtime::env::Env;

use crate::areas::{Area, area_of};
use crate::format::{BREAKING, INTENT, RISK, subject, trailer, trailers};
use crate::gitio::git;
use crate::note::NOTES_REF;

/// One commit as an agent reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Commit {
  /// The full hash.
  pub(crate) sha: String,
  /// The subject line.
  pub(crate) subject: String,
  /// The Conventional Commit type, when the subject has one.
  pub(crate) kind: Option<String>,
  /// The scope, when the subject has one.
  pub(crate) scope: Option<String>,
  /// The `Intent:` trailer.
  pub(crate) intent: Option<String>,
  /// The `Risk:` trailer.
  pub(crate) risk: Option<String>,
  /// The `Refs:` trailer, split at commas.
  pub(crate) refs: Vec<String>,
  /// The `BREAKING-CHANGE:` trailer.
  pub(crate) breaking: Option<String>,
  /// The areas it changed.
  pub(crate) areas: Vec<Area>,
  /// The branch its provenance note names.
  pub(crate) branch: Option<String>,
}

impl Commit {
  /// A parsed subject and an intent: the commit explains itself.
  pub(crate) fn semantic(&self) -> bool {
    self.kind.is_some() && self.intent.is_some()
  }
}

fn parse(sha: &str, body: &str, note: &str) -> Commit {
  let first = body.lines().next().unwrap_or_default().trim();
  let parsed = subject(first);
  let found = trailers(body);
  let value = |key: &str| {
    trailer(&found, key)
      .filter(|v| !v.is_empty())
      .map(str::to_owned)
  };
  let refs = value("Refs").map_or_else(Vec::new, |refs| {
    refs
      .split(',')
      .map(|one| one.trim().to_owned())
      .filter(|one| !one.is_empty())
      .collect()
  });
  let note: Option<Value> = serde_json::from_str(note.trim()).ok();
  Commit {
    sha: sha.to_owned(),
    subject: first.to_owned(),
    kind: parsed.map(|found| found.kind.to_owned()),
    scope: parsed.and_then(|found| found.scope).map(str::to_owned),
    intent: value(INTENT),
    risk: value(RISK),
    refs,
    breaking: value(BREAKING),
    areas: Vec::new(),
    branch: note.and_then(|note| note.get("branch")?.as_str().map(str::to_owned)),
  }
}

/// `added deleted path` lines per commit, from `--numstat` records.
fn numstat(text: &str, roots: &[String]) -> BTreeMap<String, Vec<Area>> {
  let mut found = BTreeMap::new();
  for record in text
    .split('\x1e')
    .filter(|record| !record.trim().is_empty())
  {
    let mut lines = record.lines();
    let sha = lines.next().unwrap_or_default().trim().to_owned();
    let mut areas: BTreeMap<String, Area> = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
      let mut parts = line.splitn(3, '\t');
      let (added, deleted, path) = (parts.next(), parts.next(), parts.next());
      let Some(path) = path else { continue };
      let name = area_of(path.trim_matches('"'), roots);
      let area = areas.entry(name.clone()).or_insert_with(|| Area {
        name,
        ..Area::default()
      });
      area.files += 1;
      area.added += added
        .and_then(|count| count.parse::<u64>().ok())
        .unwrap_or_default();
      area.deleted += deleted
        .and_then(|count| count.parse::<u64>().ok())
        .unwrap_or_default();
    }
    found.insert(sha, areas.into_values().collect());
  }
  found
}

/// The newest `limit` commits of `range`, newest first.
///
/// # Errors
/// When git cannot read the range.
pub(crate) fn history(
  env: &Env,
  dir: &Path,
  range: &str,
  limit: usize,
  roots: &[String],
) -> Result<Vec<Commit>, String> {
  let count = format!("--max-count={limit}");
  let notes = format!("--notes={NOTES_REF}");
  let format = [
    "log",
    &count,
    &notes,
    "--format=%H%x1f%B%x1f%N%x1e",
    range,
    "--",
  ];
  let text = git(env, dir, &format, &[])?;
  let stat_args = [
    "-c",
    "core.quotePath=false",
    "log",
    &count,
    "--no-renames",
    "--numstat",
    "--format=%x1e%H",
    range,
    "--",
  ];
  let mut areas = numstat(&git(env, dir, &stat_args, &[])?, roots);
  let mut commits = Vec::new();
  for record in text
    .split('\x1e')
    .filter(|record| !record.trim().is_empty())
  {
    let mut fields = record.trim_start_matches('\n').split('\x1f');
    let (sha, body, note) = (fields.next(), fields.next(), fields.next());
    let (Some(sha), Some(body)) = (sha, body) else {
      continue;
    };
    let mut commit = parse(sha.trim(), body, note.unwrap_or_default());
    commit.areas = areas.remove(&commit.sha).unwrap_or_default();
    commits.push(commit);
  }
  Ok(commits)
}

/// The areas of `commits` summed.
pub(crate) fn union(commits: &[Commit]) -> Vec<Area> {
  let mut all: BTreeMap<String, Area> = BTreeMap::new();
  for area in commits.iter().flat_map(|commit| &commit.areas) {
    let entry = all.entry(area.name.clone()).or_insert_with(|| Area {
      name: area.name.clone(),
      ..Area::default()
    });
    entry.files += area.files;
    entry.added += area.added;
    entry.deleted += area.deleted;
  }
  all.into_values().collect()
}

/// The highest `Risk:` among `commits`.
pub(crate) fn highest_risk(commits: &[Commit]) -> Option<&'static str> {
  ["high", "medium", "low"].into_iter().find(|level| {
    commits
      .iter()
      .any(|commit| commit.risk.as_deref() == Some(*level))
  })
}

#[cfg(test)]
#[path = "tests/history_test.rs"]
mod tests;
