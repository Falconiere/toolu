//! Pipeline stage owned by #664: deterministic scanners that run before the model.
//! Secrets (known patterns plus entropy) and ast-grep rules scan the shaped
//! committed diff and return `source: secrets | ast-grep` findings. Downstream
//! stages (#666) must treat them as trusted and non-dismissible: neither the model
//! nor Jev may drop them. Each secret finding carries `redaction_spans` into the
//! unmasked line and an already-masked `quote`, so cleartext never leaves here.

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
use crate::contract::Finding;
use crate::shape::Shaped;
pub use mechanical::mechanical_checks_lines;

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

/// What the scanners found. Every finding is trusted: `source` is `ast-grep` or
/// `secrets`, and later stages must not dismiss them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrepassResult {
  /// Capped findings, secrets first, each group worst first.
  pub findings: Vec<Finding>,
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
  let (secrets, mut notices) = secrets::scan(shaped);
  let (structural, found) = astgrep::scan(env, root, shaped, scan);
  notices.extend(found);
  let (findings, overflow) = cap::cap(secrets, structural, PREPASS_FINDINGS_MAX);
  let capped = overflow.is_some();
  notices.extend(overflow);
  PrepassResult {
    findings,
    notices,
    overflow: capped,
  }
}

/// The run-stable id of a pre-pass finding.
fn finding_id(tag: &str, path: &str, line: u32, category: &str) -> String {
  format!("prepass:{tag}:{path}:{line}:{category}")
}

#[cfg(test)]
#[path = "tests/prepass_test.rs"]
mod tests;
