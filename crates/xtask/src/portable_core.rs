//! `cargo xtask check-portable-core`: the portable-core document checklist.

use std::path::Path;

use crate::options::Options;
use crate::{Verdict, output};

const HEADINGS: [&str; 8] = [
  "## Pins",
  "## Package boundaries",
  "## Zod boundary rules",
  "## Decision contract",
  "## Event vocabulary",
  "## Policy split",
  "## Protected-files gate trace",
  "## Release blockers",
];

const CITATIONS: [(&str, &str); 6] = [
  ("dispatchPreTool", "missing native dispatch contract"),
  (
    "gates/protected-files.ts",
    "missing protected-files gate citation",
  ),
  ("gate-mode.sh", "missing gate-mode.sh citation"),
  ("dispatch.sh", "missing dispatch.sh citation"),
  (
    "fixtures/portable-core/protected-files-pre.json",
    "missing fixture citation",
  ),
  ("deny", "missing deny mapping language"),
];

const TOKENS: [&str; 4] = ["shell-out", "port-native", "port-new", "no-map"];

/// Check `docs/portable-core.md` under `--root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let doc = options.root.join("docs/portable-core.md");
  Ok(report(problems(&options.root, &doc)?))
}

pub(crate) fn problems(root: &Path, doc_path: &Path) -> Result<Option<String>, String> {
  if !doc_path.is_file() {
    return Ok(Some(format!("missing {}", doc_path.display())));
  }
  let fixture = root.join("fixtures/portable-core/protected-files-pre.json");
  if !fixture.is_file() {
    return Ok(Some(format!("missing {}", fixture.display())));
  }
  let doc = std::fs::read_to_string(doc_path)
    .map_err(|err| format!("cannot read {}: {err}", doc_path.display()))?;
  if let Some(heading) = HEADINGS.iter().find(|heading| !doc.contains(**heading)) {
    return Ok(Some(format!("missing heading: {heading}")));
  }
  for (needle, message) in CITATIONS {
    if !doc.contains(needle) {
      return Ok(Some(message.to_owned()));
    }
  }
  Ok(policy(&doc))
}

fn policy(doc: &str) -> Option<String> {
  let section = policy_section(doc);
  if section.is_empty() {
    return Some("empty Policy split section".to_owned());
  }
  for token in TOKENS {
    if !section.contains(token) {
      return Some(format!("missing classification token: {token}"));
    }
  }
  if section.lines().any(maybe_later) {
    return Some("invalid classification token maybe-later".to_owned());
  }
  None
}

fn policy_section(doc: &str) -> String {
  let mut inside = false;
  let mut lines = Vec::new();
  for line in doc.lines() {
    if line.starts_with("## Policy split") {
      inside = true;
    } else if line.starts_with("## ") {
      inside = false;
    } else if inside {
      lines.push(line);
    }
  }
  lines.join("\n")
}

fn maybe_later(line: &str) -> bool {
  if line.contains("`maybe-later`") {
    return true;
  }
  line
    .trim_start()
    .strip_prefix('-')
    .is_some_and(|rest| rest.trim_start().starts_with("maybe-later"))
}

fn report(problem: Option<String>) -> Verdict {
  if let Some(message) = problem {
    output::error(&format!("check-portable-core-doc: {message}"));
    return Verdict::Findings;
  }
  output::say("check-portable-core-doc: ok");
  Verdict::Clean
}

#[cfg(test)]
#[path = "tests/portable_core_test.rs"]
mod tests;
