//! `cargo xtask check-portable-core`: the portable-core document checklist.

use std::path::Path;

use serde_json::Value;

use crate::options::Options;
use crate::{Verdict, output};

const HEADINGS: [&str; 11] = [
  "## Pins",
  "## Package boundaries",
  "## Zod boundary rules",
  "## Decision contract",
  "## Event vocabulary",
  "## OpenCode dispatch contract",
  "## Policy split",
  "## Protected-files gate trace",
  "## OpenCode interception",
  "### Capability-results",
  "## Release blockers",
];

const CITATIONS: [(&str, &str); 7] = [
  ("opencode-host-contract.md", "missing host contract link"),
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
const V2_DOCS: &str = "opencode.ai/v2/";

struct Pin {
  cli: String,
  sdk: String,
}

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
  let pin = match read_pin(&root.join("tools/toolu-opencode/contract/pin.json")) {
    Ok(pin) => pin,
    Err(message) => return Ok(Some(message)),
  };
  let needles = [
    (pin.cli.as_str(), format!("missing CLI pin {}", pin.cli)),
    (pin.sdk.as_str(), "missing SDK pin".to_owned()),
  ];
  for (needle, message) in needles {
    if !doc.contains(needle) {
      return Ok(Some(message));
    }
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
  if doc.contains(V2_DOCS) {
    return Some(format!("cites the V2 contract ({V2_DOCS})"));
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

fn read_pin(path: &Path) -> Result<Pin, String> {
  let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
  let value: Value =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
  let obj = value
    .as_object()
    .ok_or_else(|| format!("{}: pin schema", path.display()))?;
  for key in ["version", "cli", "sdk", "docs"] {
    if !obj.contains_key(key) {
      return Err(format!("{}: pin schema", path.display()));
    }
  }
  if obj.len() != 4 || value.get("version") != Some(&serde_json::json!(1)) {
    return Err(format!("{}: pin schema", path.display()));
  }
  let docs = value.get("docs").and_then(Value::as_str).unwrap_or("");
  if !docs.starts_with("https://") && !docs.starts_with("http://") {
    return Err(format!("{}: pin schema", path.display()));
  }
  Ok(Pin {
    cli: pin_part(&value, "cli", "opencode-ai", path)?,
    sdk: pin_part(&value, "sdk", "@opencode-ai/plugin", path)?,
  })
}

fn pin_part(value: &Value, key: &str, package: &str, path: &Path) -> Result<String, String> {
  let part = value
    .get(key)
    .ok_or_else(|| format!("{}: pin schema", path.display()))?;
  let got = part.get("package").and_then(Value::as_str);
  let version = part.get("version").and_then(Value::as_str).unwrap_or("");
  let keys = part.as_object().map_or(0, serde_json::Map::len);
  if got != Some(package) || keys != 2 || !semver(version) {
    return Err(format!("{}: pin schema", path.display()));
  }
  Ok(format!("{package}@{version}"))
}

fn semver(text: &str) -> bool {
  let mut parts = text.split('.');
  let three = parts.by_ref().take(3).collect::<Vec<_>>();
  three.len() == 3
    && parts.next().is_none()
    && three
      .iter()
      .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
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
