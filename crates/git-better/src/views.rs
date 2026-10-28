//! How commits, areas and the branch context read: compact text an agent can
//! skim, and the JSON the `--json` documents carry.

use serde_json::{Value, json};

use crate::areas::Area;
use crate::branch::parse;
use crate::context::Context;
use crate::history::{Commit, highest_risk, union};

/// At most this many areas are listed in text.
const TEXT_AREAS: usize = 8;

/// One area as JSON.
pub(crate) fn area_json(area: &Area) -> Value {
  json!({ "area": area.name, "files": area.files, "added": area.added, "deleted": area.deleted })
}

/// One area as text: `crates/x (+12 -3, 4 files)`.
pub(crate) fn area_text(area: &Area) -> String {
  let files = if area.files == 1 { "file" } else { "files" };
  format!(
    "{} (+{} -{}, {} {files})",
    area.name, area.added, area.deleted, area.files
  )
}

/// Areas as one line, the largest first, at most [`TEXT_AREAS`].
pub(crate) fn areas_line(areas: &[Area]) -> String {
  let mut sorted: Vec<&Area> = areas.iter().collect();
  sorted.sort_by_key(|area| std::cmp::Reverse(area.added + area.deleted));
  let mut shown: Vec<String> = sorted
    .iter()
    .take(TEXT_AREAS)
    .map(|area| area_text(area))
    .collect();
  if sorted.len() > TEXT_AREAS {
    shown.push(format!("{} more", sorted.len() - TEXT_AREAS));
  }
  shown.join(", ")
}

/// One commit as JSON.
pub(crate) fn commit_json(commit: &Commit) -> Value {
  json!({
    "sha": commit.sha,
    "subject": commit.subject,
    "type": commit.kind,
    "scope": commit.scope,
    "semantic": commit.semantic(),
    "intent": commit.intent,
    "risk": commit.risk,
    "refs": commit.refs,
    "breaking": commit.breaking,
    "areas": commit.areas.iter().map(area_json).collect::<Vec<_>>(),
    "branch": commit.branch,
  })
}

/// One commit as text: its short hash and subject, then why and where.
pub(crate) fn commit_text(commit: &Commit) -> String {
  let short = commit.sha.get(..8).unwrap_or(&commit.sha);
  let mut lines = vec![format!("{short} {}", commit.subject)];
  match &commit.intent {
    Some(intent) => lines.push(format!("  why: {intent}")),
    None => lines.push("  why: (no Intent: trailer)".to_owned()),
  }
  if !commit.areas.is_empty() {
    lines.push(format!("  where: {}", areas_line(&commit.areas)));
  }
  if let Some(risk) = &commit.risk {
    lines.push(format!("  risk: {risk}"));
  }
  if let Some(breaking) = &commit.breaking {
    lines.push(format!("  breaking: {breaking}"));
  }
  lines.join("\n")
}

/// `n` and `noun`, plural unless `n` is 1.
fn count(n: usize, noun: &str) -> String {
  if n == 1 {
    format!("1 {noun}")
  } else {
    format!("{n} {noun}s")
  }
}

/// The branch as words: `feat/653-pause-it` reads `feat, ref 653: pause it`.
fn branch_words(name: &str) -> Option<String> {
  let branch = parse(name)?;
  let words = branch.slug.replace('-', " ");
  Some(match branch.reference {
    Some(reference) => format!("{}, ref {reference}: {words}", branch.kind),
    None => format!("{}: {words}", branch.kind),
  })
}

/// The refs of every commit, first seen first.
pub(crate) fn all_refs(commits: &[Commit]) -> Vec<String> {
  let mut refs: Vec<String> = Vec::new();
  for one in commits.iter().flat_map(|commit| &commit.refs) {
    if !refs.contains(one) {
      refs.push(one.clone());
    }
  }
  refs
}

/// The lines about a feature branch's commits: areas, risk, breaking changes,
/// refs and each intent.
fn branch_lines(context: &Context) -> Vec<String> {
  let mut lines = Vec::new();
  let areas = union(&context.commits);
  if !areas.is_empty() {
    lines.push(format!("Areas: {}", areas_line(&areas)));
  }
  let risk = highest_risk(&context.commits).unwrap_or("not rated");
  lines.push(format!("Highest risk: {risk}"));
  let breaking: Vec<&str> = context
    .commits
    .iter()
    .filter_map(|c| c.breaking.as_deref())
    .collect();
  if !breaking.is_empty() {
    lines.push(format!("Breaking: {}", breaking.join("; ")));
  }
  let refs = all_refs(&context.commits);
  if !refs.is_empty() {
    lines.push(format!("Refs: {}", refs.join(", ")));
  }
  let intents: Vec<String> = context
    .commits
    .iter()
    .filter_map(|commit| Some(format!("- {}: {}", commit.subject, commit.intent.as_ref()?)))
    .collect();
  if !intents.is_empty() {
    lines.push("Why, newest first:".to_owned());
    lines.extend(intents);
  }
  lines
}

/// The context as text.
pub(crate) fn context_text(context: &Context) -> String {
  let parsed =
    branch_words(&context.branch).map_or_else(String::new, |words| format!(" ({words})"));
  let mut lines = Vec::new();
  if context.on_base {
    lines.push(format!(
      "Branch {}{parsed}: the base branch.",
      context.branch
    ));
  } else {
    let semantic = context
      .commits
      .iter()
      .filter(|commit| commit.semantic())
      .count();
    lines.push(format!(
      "Branch {}{parsed}: {} ahead of {}, {semantic} semantic.",
      context.branch,
      count(context.commits.len(), "commit"),
      context.base
    ));
    lines.extend(branch_lines(context));
  }
  let dirty = context.dirty;
  lines.push(format!(
    "Working tree: {} staged, {} unstaged, {} untracked.",
    dirty.staged, dirty.unstaged, dirty.untracked
  ));
  lines.join("\n")
}

/// The context as JSON.
pub(crate) fn context_json(context: &Context) -> Value {
  let breaking: Vec<&str> = context
    .commits
    .iter()
    .filter_map(|c| c.breaking.as_deref())
    .collect();
  json!({
    "namespace": crate::PLUGIN,
    "verb": "context",
    "branch": context.branch,
    "base": context.base,
    "range": context.range,
    "onBase": context.on_base,
    "commits": context.commits.iter().map(commit_json).collect::<Vec<_>>(),
    "areas": union(&context.commits).iter().map(area_json).collect::<Vec<_>>(),
    "risk": highest_risk(&context.commits),
    "breaking": breaking,
    "refs": all_refs(&context.commits),
    "dirty": {
      "staged": context.dirty.staged,
      "unstaged": context.dirty.unstaged,
      "untracked": context.dirty.untracked,
    },
  })
}

#[cfg(test)]
#[path = "tests/views_test.rs"]
mod tests;
