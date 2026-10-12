//! Pick the tracker for an epic reference (`scripts/trackers/index.ts`). URLs
//! and `jira:`/`linear:` prefixes are unambiguous; a bare `ABC-12` goes to
//! whichever of Jira or Linear is configured, and asks for a prefix when both are.

use regex::{Regex, RegexBuilder};
use toolu_runtime::env::Env;

/// The three trackers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackerKind {
  /// GitHub issues.
  Github,
  /// Jira.
  Jira,
  /// Linear.
  Linear,
}

impl TrackerKind {
  /// The `graph.json` `tracker` value.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Github => "github",
      Self::Jira => "jira",
      Self::Linear => "linear",
    }
  }

  /// The kind a `graph.json` `tracker` value names.
  pub(crate) fn parse(name: &str) -> Option<Self> {
    match name {
      "github" => Some(Self::Github),
      "jira" => Some(Self::Jira),
      "linear" => Some(Self::Linear),
      _ => None,
    }
  }
}

/// A Linear epic: a parent issue or a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinearRef {
  /// A parent issue identifier, `ENG-12`.
  Issue(String),
  /// A project slug id.
  Project(String),
}

fn insensitive(pattern: &str) -> Option<Regex> {
  RegexBuilder::new(pattern)
    .case_insensitive(true)
    .build()
    .ok()
}

fn matches(pattern: &str, text: &str) -> bool {
  insensitive(pattern).is_some_and(|re| re.is_match(text))
}

/// The tracker for `reference`, or the explicit `hint`.
///
/// # Errors
/// An unknown hint, an unrecognized reference, or a bare key both Jira and
/// Linear could own.
pub(crate) fn detect_tracker(
  reference: &str,
  hint: Option<&str>,
  env: &Env,
) -> Result<TrackerKind, String> {
  if let Some(hint) = hint {
    return TrackerKind::parse(hint)
      .ok_or_else(|| format!("unknown tracker {hint}; use github, jira, or linear"));
  }
  let text = reference.trim();
  if matches(r"^jira:", text) || matches(r"atlassian\.net/browse/", text) {
    return Ok(TrackerKind::Jira);
  }
  if matches(r"^linear:", text) || matches(r"linear\.app/", text) {
    return Ok(TrackerKind::Linear);
  }
  if Regex::new(r"github\.com/|^[\w.-]+/[\w.-]+#\d+$|^#?\d+$").is_ok_and(|re| re.is_match(text)) {
    return Ok(TrackerKind::Github);
  }
  if parse_jira_ref(text).is_some() || parse_linear_ref(text).is_some() {
    let jira = env.get("JIRA_BASE_URL").is_some();
    let linear = env.get("LINEAR_API_KEY").is_some();
    return match (jira, linear) {
      (true, false) => Ok(TrackerKind::Jira),
      (false, true) => Ok(TrackerKind::Linear),
      _ => Err(format!(
        "{text} could be a Jira or Linear key; prefix it (jira:{text} or linear:{text}) or pass --tracker"
      )),
    };
  }
  Err(format!(
    "unrecognized epic reference: {}",
    serde_json::to_string(reference).unwrap_or_default()
  ))
}

/// A Jira key from `KEY-1`, `jira:key-1` or a browse URL.
pub(crate) fn parse_jira_ref(reference: &str) -> Option<String> {
  let text = strip(reference.trim(), "jira:");
  if let Some(found) = insensitive(r"/browse/([A-Z][A-Z0-9_]+-\d+)")
    .and_then(|re| re.captures(text).and_then(|caps| caps.get(1)))
  {
    return Some(found.as_str().to_uppercase());
  }
  let upper = text.to_uppercase();
  let key = Regex::new(r"^[A-Z][A-Z0-9_]+-\d+$").is_ok_and(|re| re.is_match(&upper));
  (key && text.starts_with(|ch: char| ch.is_ascii_alphabetic())).then_some(upper)
}

/// A Linear issue or project from its URL, a prefix form or a bare identifier.
pub(crate) fn parse_linear_ref(reference: &str) -> Option<LinearRef> {
  let text = strip(reference.trim(), "linear:");
  if let Some(found) = insensitive(r"linear\.app/[^/\s]+/issue/([A-Z][A-Z0-9]*-\d+)")
    .and_then(|re| re.captures(text).and_then(|caps| caps.get(1)))
  {
    return Some(LinearRef::Issue(found.as_str().to_uppercase()));
  }
  if let Some(found) = insensitive(r"linear\.app/[^/\s]+/project/([\w-]+)")
    .and_then(|re| re.captures(text).and_then(|caps| caps.get(1)))
  {
    let id = found.as_str().split('-').rfind(|part| !part.is_empty());
    if let Some(id) = id {
      return Some(LinearRef::Project(id.to_owned()));
    }
  }
  if let Some(found) = insensitive(r"^project[:/]").and_then(|re| re.find(text)) {
    return text
      .get(found.end()..)
      .map(|rest| LinearRef::Project(rest.to_owned()));
  }
  let upper = text.to_uppercase();
  Regex::new(r"^[A-Z][A-Z0-9]*-\d+$")
    .is_ok_and(|re| re.is_match(&upper))
    .then_some(LinearRef::Issue(upper))
}

fn strip<'a>(text: &'a str, prefix: &str) -> &'a str {
  match text.get(..prefix.len()) {
    Some(head) if head.eq_ignore_ascii_case(prefix) => text.get(prefix.len()..).unwrap_or(""),
    _ => text,
  }
}

#[cfg(test)]
#[path = "tests/detect_test.rs"]
mod tests;
