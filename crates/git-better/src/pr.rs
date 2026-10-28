//! A draft PR title and body, and a squash message, from the branch's commits:
//! the intents become the summary and the trailers merge, so a squash merge
//! keeps what each commit said. A draft for the agent to edit, not to trust.

use std::collections::BTreeMap;

use crate::branch::parse;
use crate::context::Context;
use crate::history::{Commit, highest_risk, union};
use crate::views::{all_refs, area_text};

/// What `pr` writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
  /// The PR title, also the squash subject.
  pub(crate) title: String,
  /// The PR body, in Markdown.
  pub(crate) body: String,
  /// The squash commit message, trailers merged.
  pub(crate) squash: String,
}

/// The most frequent value, the first seen on a tie.
fn most_common<'a>(values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
  let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
  for (order, value) in values.enumerate() {
    let entry = counts.entry(value).or_insert((0, order));
    entry.0 += 1;
  }
  counts
    .into_iter()
    .max_by(|(_, (a, first_a)), (_, (b, first_b))| a.cmp(b).then(first_b.cmp(first_a)))
    .map(|(value, _)| value)
}

fn description(subject: &str) -> &str {
  subject.split_once(": ").map_or(subject, |(_, rest)| rest)
}

fn title(context: &Context, semantic: &[&Commit]) -> String {
  if let [only] = semantic {
    return only.subject.clone();
  }
  let branch = parse(&context.branch);
  let kind = branch
    .map(|branch| branch.kind)
    .or_else(|| most_common(semantic.iter().filter_map(|c| c.kind.as_deref())))
    .unwrap_or("<type>");
  let scope = most_common(semantic.iter().filter_map(|c| c.scope.as_deref())).unwrap_or("<area>");
  let bang = if semantic.iter().any(|commit| commit.breaking.is_some()) {
    "!"
  } else {
    ""
  };
  let words = branch.map(|branch| branch.slug.replace('-', " "));
  let what = words
    .or_else(|| {
      semantic
        .last()
        .map(|commit| description(&commit.subject).to_owned())
    })
    .unwrap_or_else(|| "<what changed>".to_owned());
  format!("{kind}({scope}){bang}: {what}")
}

fn body(context: &Context, oldest: &[&Commit]) -> String {
  let mut lines = vec!["## Summary".to_owned(), String::new()];
  for commit in oldest {
    lines.push(match &commit.intent {
      Some(intent) => format!("- {intent} (`{}`)", commit.subject),
      None => format!("- `{}`", commit.subject),
    });
  }
  let mut areas = union(&context.commits);
  areas.sort_by_key(|area| std::cmp::Reverse(area.added + area.deleted));
  if !areas.is_empty() {
    lines.extend([String::new(), "## Areas".to_owned(), String::new()]);
    lines.extend(areas.iter().map(|area| format!("- {}", area_text(area))));
  }
  let risk = highest_risk(&context.commits).unwrap_or("not rated");
  lines.extend([
    String::new(),
    "## Risk".to_owned(),
    String::new(),
    risk.to_owned(),
  ]);
  let breaking: Vec<&str> = oldest
    .iter()
    .filter_map(|c| c.breaking.as_deref())
    .collect();
  if !breaking.is_empty() {
    lines.extend([
      String::new(),
      "## Breaking changes".to_owned(),
      String::new(),
    ]);
    lines.extend(breaking.iter().map(|text| format!("- {text}")));
  }
  let reference = parse(&context.branch)
    .and_then(|branch| branch.reference)
    .filter(|reference| reference.bytes().all(|b| b.is_ascii_digit()));
  if let Some(number) = reference {
    lines.extend([String::new(), format!("Closes #{number}")]);
  }
  if context.commits.len() < context.total {
    let read = context.commits.len();
    lines.extend([
      String::new(),
      format!(
        "_Drafted from the newest {read} of {} commits._",
        context.total
      ),
    ]);
  }
  lines.join("\n")
}

fn squash(title: &str, context: &Context, oldest: &[&Commit]) -> String {
  let mut lines = vec![title.to_owned(), String::new()];
  lines.extend(oldest.iter().map(|commit| format!("- {}", commit.subject)));
  let intents: Vec<&str> = oldest.iter().filter_map(|c| c.intent.as_deref()).collect();
  let intent = if intents.is_empty() {
    "<why this change is needed>".to_owned()
  } else {
    intents.join("; ")
  };
  lines.extend([String::new(), format!("Intent: {intent}")]);
  if let Some(risk) = highest_risk(&context.commits) {
    lines.push(format!("Risk: {risk}"));
  }
  let refs = all_refs(&context.commits);
  if !refs.is_empty() {
    lines.push(format!("Refs: {}", refs.join(", ")));
  }
  let breaking: Vec<&str> = oldest
    .iter()
    .filter_map(|c| c.breaking.as_deref())
    .collect();
  if !breaking.is_empty() {
    lines.push(format!("BREAKING-CHANGE: {}", breaking.join("; ")));
  }
  lines.join("\n")
}

/// The draft for the branch in `context`.
pub(crate) fn draft(context: &Context) -> Draft {
  let skipped = |commit: &&Commit| {
    commit.kind.as_deref() == Some("wip")
      || ["fixup! ", "squash! ", "amend! "]
        .iter()
        .any(|prefix| commit.subject.starts_with(prefix))
  };
  let oldest: Vec<&Commit> = context
    .commits
    .iter()
    .rev()
    .filter(|c| !skipped(c))
    .collect();
  let semantic: Vec<&Commit> = oldest.iter().copied().filter(|c| c.semantic()).collect();
  let title = title(context, &semantic);
  Draft {
    body: body(context, &oldest),
    squash: squash(&title, context, &oldest),
    title,
  }
}

#[cfg(test)]
#[path = "tests/pr_test.rs"]
mod tests;
