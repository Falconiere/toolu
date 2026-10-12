//! The coupling check of a launch batch (skill step 1): two issues that change
//! the same module, schema or migration would force one to rebase over the
//! other, so the later one is held. Jev judges each pair when it is
//! available; otherwise shared path tokens, migration identifiers and
//! `module:`/`component:`/`path:` labels decide.

use std::collections::BTreeSet;

use regex::Regex;
use serde_json::{Map, Value, json};
use toolu_jev_client::{Answer, Jev, Questions, State};

use crate::graph::GraphIssue;

/// Jev's probability at or above which a pair is held (between its 0.75 and
/// 0.90 split).
pub(crate) const HOLD_AT: f64 = 0.80;

/// A hold on the candidate.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hold {
  /// The member it overlaps.
  pub with: String,
  /// Jev's probability, or 1 for the heuristic.
  pub score: f64,
  /// `jev` or `heuristic`.
  pub source: &'static str,
}

/// The check's outcome, and why Jev was not used when it failed.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Verdict {
  /// The hold, if any.
  pub hold: Option<Hold>,
  /// The Jev failure the heuristic replaced.
  pub fallback: Option<String>,
}

fn text_of(issue: &GraphIssue) -> String {
  format!("{}\n{}", issue.title, issue.excerpt)
}

/// File-like paths: two or more `/`, or a last segment with an extension.
/// URLs are not paths, so `and/or`, `CI/CD` and links never match.
pub(crate) fn path_tokens(text: &str) -> BTreeSet<String> {
  let (Ok(url), Ok(path)) = (
    Regex::new(r"[A-Za-z][A-Za-z0-9+.-]*://\S+"),
    Regex::new(r"[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+"),
  ) else {
    return BTreeSet::new();
  };
  let plain = url.replace_all(text, " ");
  path
    .find_iter(&plain)
    .map(|found| found.as_str().trim_end_matches('.').to_owned())
    .filter(|token| {
      let last = token.rsplit('/').next().unwrap_or("");
      let extension = last
        .find('.')
        .is_some_and(|dot| dot > 0 && dot + 1 < last.len());
      extension || token.matches('/').count() >= 2
    })
    .collect()
}

/// Migration identifiers: `0042_add_users` or `V3__init`.
pub(crate) fn migrations(text: &str) -> BTreeSet<String> {
  let Ok(migration) = Regex::new(r"\b(?:\d{3,}[_-][A-Za-z0-9_-]+|V\d+__[A-Za-z0-9_]+)") else {
    return BTreeSet::new();
  };
  migration
    .find_iter(text)
    .map(|found| found.as_str().to_owned())
    .collect()
}

fn scoped_labels(issue: &GraphIssue) -> BTreeSet<String> {
  issue
    .labels
    .iter()
    .filter(|label| {
      ["module:", "component:", "path:"]
        .iter()
        .any(|prefix| label.starts_with(prefix))
    })
    .cloned()
    .collect()
}

/// Why two issues overlap without Jev, if they do.
pub(crate) fn heuristic(a: &GraphIssue, b: &GraphIssue) -> Option<String> {
  let (left, right) = (text_of(a), text_of(b));
  if let Some(path) = path_tokens(&left).intersection(&path_tokens(&right)).next() {
    return Some(format!("shared path {path}"));
  }
  if let Some(migration) = migrations(&left).intersection(&migrations(&right)).next() {
    return Some(format!("shared migration {migration}"));
  }
  scoped_labels(a)
    .intersection(&scoped_labels(b))
    .next()
    .map(|label| format!("shared label {label}"))
}

fn evidence(issue: &GraphIssue) -> Value {
  json!({"title": issue.title, "labels": issue.labels, "description_excerpt": issue.excerpt})
}

/// Jev's probability per member that it overlaps `candidate`.
///
/// # Errors
/// The call fails.
pub(crate) fn jev_pairs(
  jev: &Jev,
  candidate: &GraphIssue,
  ahead: &[&GraphIssue],
) -> Result<Vec<(String, f64)>, String> {
  let mut issues = Map::new();
  let mut questions = Map::new();
  issues.insert(candidate.key.clone(), evidence(candidate));
  for member in ahead {
    issues.insert(member.key.clone(), evidence(member));
    let instructions = format!(
      "Do `issues.{}` and `issues.{}` change the same module, schema or migration so that working them in parallel would force one to rebase over the other?",
      member.key, candidate.key
    );
    questions.insert(
      member.key.clone(),
      json!({"type": "noul", "instructions": instructions}),
    );
  }
  let asked =
    Questions::parse(&Value::Object(questions).to_string()).map_err(|err| err.to_string())?;
  let state = State::structured(&json!({"issues": issues}).to_string());
  let reply = jev
    .ask(&state, toolu_jev_client::DEFAULT_MODEL, &asked)
    .map_err(|err| format!("jev failed: {err}"))?;
  Ok(
    ahead
      .iter()
      .filter_map(|member| {
        match reply
          .answers
          .iter()
          .find(|(key, _)| *key == member.key)
          .map(|(_, answer)| answer)
        {
          Some(Answer::Noul { noul }) => Some((member.key.clone(), *noul)),
          Some(Answer::Choice { .. } | Answer::Score { .. }) | None => None,
        }
      })
      .collect(),
  )
}

/// Hold `candidate` behind the first member `ahead` (in batch order) it
/// overlaps. `jev` is `None` without `TYPESAFE_API_KEY`.
pub(crate) fn check(
  candidate: &GraphIssue,
  ahead: &[&GraphIssue],
  jev: Option<Result<&Jev, String>>,
) -> Verdict {
  if ahead.is_empty() {
    return Verdict::default();
  }
  let fallback = match jev {
    Some(Ok(jev)) => match jev_pairs(jev, candidate, ahead) {
      Ok(scores) => {
        let hold = scores
          .into_iter()
          .find(|(_, score)| *score >= HOLD_AT)
          .map(|(with, score)| Hold {
            with,
            score,
            source: "jev",
          });
        return Verdict {
          hold,
          fallback: None,
        };
      }
      Err(reason) => Some(reason),
    },
    Some(Err(reason)) => Some(reason),
    None => None,
  };
  let hold = ahead
    .iter()
    .find(|member| heuristic(candidate, member).is_some())
    .map(|member| Hold {
      with: member.key.clone(),
      score: 1.0,
      source: "heuristic",
    });
  Verdict { hold, fallback }
}

#[cfg(test)]
#[path = "tests/coupling_test.rs"]
mod tests;
