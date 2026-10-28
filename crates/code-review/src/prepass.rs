//! Pipeline stage owned by #664: deterministic scanners that run before the model.
//! Secrets (known patterns plus entropy) and ast-grep rules scan the shaped
//! committed diff and return `source: secrets | ast-grep` findings. Downstream
//! stages (#666) must treat them as trusted and non-dismissible: neither the model
//! nor Jev may drop them. Each secret finding carries `redaction_spans` into the
//! unmasked line and an already-masked `quote`, so cleartext never leaves here.
//! `redactions` lists every changed line on either side that holds a secret,
//! never capped, so later stages can mask all outbound text, removed lines too.

mod astgrep;
mod cap;
mod mechanical;
mod secrets;

use std::path::Path;

use serde::Serialize;
use toolu_engine::quality::{AstGrepScan, EditedFile, scan_inline};
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use crate::budget::PREPASS_FINDINGS_MAX;
use crate::contract::{Finding, RedactionSpan, Side, Source};
use crate::shape::Shaped;
pub use mechanical::mechanical_checks_lines;

/// `text` with every secret the scanner knows masked, line by line: for
/// repository text outside the changed lines, which the scan never sees.
pub(crate) fn redact_text(text: &str) -> String {
  text
    .split_inclusive('\n')
    .map(|line| {
      let body = line.strip_suffix('\n').unwrap_or(line);
      let (masked, _) = secrets::redact_line(body);
      if body.len() == line.len() {
        masked
      } else {
        masked + "\n"
      }
    })
    .collect()
}

/// Why the pre-pass says something besides findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoticeKind {
  /// ast-grep is not installed; structural rules did not run.
  MissingAstGrep,
  /// ast-grep, or materializing a file for it, failed.
  AstGrepFailed,
  /// Findings beyond `PREPASS_FINDINGS_MAX` were dropped.
  Overflow,
  /// The shared secrets table could not be compiled; no secrets ran.
  SecretsCompile,
}

/// One degrade or overflow line; never a silent skip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
  /// What happened.
  pub kind: NoticeKind,
  /// One human line.
  pub text: String,
}

/// The secrets in one changed line of a shaped file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Redaction {
  /// The shaped file's `path`.
  pub path: String,
  /// The side `line` numbers.
  pub side: Side,
  /// One-based line on `side`.
  pub line: u32,
  /// Byte ranges of the secrets in that line's unmasked text.
  pub spans: Vec<RedactionSpan>,
}

/// What the scanners found. Every finding is trusted: `source` is `ast-grep` or
/// `secrets`, and later stages must not dismiss them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepassResult {
  /// Capped findings, secrets first, each group worst first.
  pub findings: Vec<Finding>,
  /// Every changed line, old or new, that holds a secret; the cap never drops one.
  pub redactions: Vec<Redaction>,
  /// Degrade and overflow lines.
  pub notices: Vec<Notice>,
  /// Whether findings were dropped at the cap.
  pub overflow: bool,
}

/// The ast-grep entry point `run` uses: the engine's `scan_inline`.
type Scan<'a> = &'a dyn Fn(&[EditedFile], &str, &RuleContext<'_>) -> AstGrepScan;

/// Scan the shaped committed diff with the production engine scanner.
pub fn run(env: &Env, root: &Path, shaped: &Shaped) -> PrepassResult {
  run_with(env, root, shaped, &scan_inline)
}

/// `run` with the ast-grep scan injected, so a test can drive its `Missing` and
/// `Failed` outcomes without touching `PATH`.
pub fn run_with(env: &Env, root: &Path, shaped: &Shaped, scan: Scan<'_>) -> PrepassResult {
  let (secrets, redactions, mut notices) = secrets::scan(shaped);
  let (structural, found) = astgrep::scan(env, root, shaped, scan);
  notices.extend(found);
  let (findings, overflow) = cap::cap(secrets, structural, PREPASS_FINDINGS_MAX);
  let capped = overflow.is_some();
  notices.extend(overflow);
  PrepassResult {
    findings,
    redactions,
    notices,
    overflow: capped,
  }
}

/// The run-stable id of a pre-pass finding.
fn finding_id(source: Source, path: &str, line: u32, category: &str) -> String {
  format!("prepass:{}:{path}:{line}:{category}", source.name())
}

#[cfg(test)]
#[path = "tests/prepass_test.rs"]
mod tests;
