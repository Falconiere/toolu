use std::fs;
use std::path::Path;
use std::process::Command;

use toolu_engine::quality::{AstGrepScan, ScanFailure, ScanStage};
use toolu_runtime::env::Env;

use super::{NoticeKind, PrepassResult, run_with};
use crate::contract::{Coverage, Finding, Severity, Side, Source};
use crate::shape::{ChangedLine, FileKind, Shaped, ShapedFile, shape};

pub(super) fn token() -> String {
  format!("ghp_{}", "Ab1".repeat(12))
}

pub(super) fn added(line: u32, text: &str) -> ChangedLine {
  ChangedLine {
    side: Side::New,
    line,
    text: text.to_owned(),
  }
}

pub(super) fn shaped(path: &str, lines: Vec<ChangedLine>) -> Shaped {
  Shaped {
    base_ref: "main".to_owned(),
    merge_base: String::new(),
    diff_sha: String::new(),
    gate_files: vec![path.to_owned()],
    files: vec![ShapedFile {
      path: path.to_owned(),
      old_path: None,
      kind: FileKind::Modified,
      patch: String::new(),
      lines,
    }],
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: Vec::new(),
    uncommitted: false,
  }
}

pub(super) fn empty() -> PrepassResult {
  PrepassResult {
    findings: Vec::new(),
    notices: Vec::new(),
    overflow: false,
  }
}

pub(super) fn synthetic(source: Source, severity: Severity, index: u32) -> Finding {
  Finding {
    id: format!("f{index}"),
    path: format!("src/f{index:03}.rs"),
    line: index + 1,
    end_line: index + 1,
    side: Side::New,
    severity,
    category: "c".to_owned(),
    source,
    claim: format!("claim {index}"),
    quote: "q".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
  }
}

pub(super) fn batch(source: Source, severity: Severity, count: u32) -> Vec<Finding> {
  (0..count)
    .map(|index| synthetic(source, severity, index))
    .collect()
}

fn git(root: &Path, args: &[&str]) {
  let out = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("XDG_CONFIG_HOME", "/dev/null")
    .output()
    .expect("git starts");
  assert!(out.status.success(), "git {args:?}: {:?}", out.stderr);
}

pub(super) fn env() -> Env {
  Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null")
}

/// A feature branch that adds a line holding a secret to `src/lib.rs`.
pub(super) fn secret_repo() -> (tempfile::TempDir, Shaped) {
  let dir = tempfile::tempdir().expect("repository");
  let root = dir.path();
  git(root, &["init", "-b", "main"]);
  git(root, &["config", "user.name", "Test"]);
  git(root, &["config", "user.email", "test@example.com"]);
  fs::create_dir(root.join("src")).expect("src");
  fs::write(root.join("src/lib.rs"), "fn a() {}\n").expect("base");
  git(root, &["add", "-A"]);
  git(root, &["commit", "-m", "base"]);
  git(root, &["switch", "-c", "feature"]);
  let body = format!("fn a() {{}}\nconst T: &str = \"{}\";\n", token());
  fs::write(root.join("src/lib.rs"), body).expect("change");
  git(root, &["add", "-A"]);
  git(root, &["commit", "-m", "change"]);
  let shaped = shape(&env(), root, Some("main")).expect("shape");
  (dir, shaped)
}

#[test]
fn prepass_run_missing_astgrep_is_a_notice_and_secrets_still_run() {
  let (dir, shaped) = secret_repo();
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| AstGrepScan::Missing);
  let kinds: Vec<NoticeKind> = result.notices.iter().map(|notice| notice.kind).collect();
  assert_eq!(kinds, [NoticeKind::MissingAstGrep]);
  assert_eq!(
    result.notices[0].text,
    "prepass: ast-grep not installed; structural rules skipped"
  );
  assert_eq!(result.findings.len(), 1);
  assert_eq!(result.findings[0].source, Source::Secrets);
  assert!(!result.overflow);
}

#[test]
fn prepass_run_failed_astgrep_names_the_failure_and_secrets_still_run() {
  let (dir, shaped) = secret_repo();
  let failed = AstGrepScan::Failed(ScanFailure {
    stage: ScanStage::AstGrep,
    exit_code: 2,
    stderr_first: "bad rule".to_owned(),
  });
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| failed.clone());
  assert_eq!(result.notices.len(), 1);
  assert_eq!(result.notices[0].kind, NoticeKind::AstGrepFailed);
  assert_eq!(
    result.notices[0].text,
    "prepass: ast-grep failed (exit 2): bad rule"
  );
  assert_eq!(result.findings.len(), 1);
}
