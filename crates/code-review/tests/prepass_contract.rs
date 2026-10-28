//! Black-box pre-pass contract: the public API against a real committed Git diff,
//! with the real `ast-grep` on `PATH`.

use std::fs;
use std::path::Path;
use std::process::Command;

use toolu_code_review::budget::PREPASS_FINDINGS_MAX;
use toolu_code_review::contract::{Finding, RedactionSpan, Severity, Side, Source};
use toolu_code_review::prepass::{
  Notice, NoticeKind, PrepassResult, mechanical_checks_lines, run, run_with,
};
use toolu_code_review::shape::shape;
use toolu_engine::quality::AstGrepScan;
use toolu_runtime::env::Env;

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
  let out = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("XDG_CONFIG_HOME", "/dev/null")
    .output()
    .map_err(|err| format!("git {args:?}: {err}"))?;
  if !out.status.success() {
    return Err(format!(
      "git {args:?}: {}",
      String::from_utf8_lossy(&out.stderr)
    ));
  }
  Ok(())
}

fn env() -> Env {
  Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null")
}

/// `base` on `main`, then `change` committed on `feature`, both as `src/lib.rs`.
fn repo(base: &str, change: &str) -> Result<tempfile::TempDir, String> {
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let root = dir.path();
  let write = |text: &str| fs::write(root.join("src/lib.rs"), text).map_err(|err| err.to_string());
  git(root, &["init", "-b", "main"])?;
  git(root, &["config", "user.name", "Test"])?;
  git(root, &["config", "user.email", "test@example.com"])?;
  fs::create_dir(root.join("src")).map_err(|err| err.to_string())?;
  write(base)?;
  git(root, &["add", "-A"])?;
  git(root, &["commit", "-m", "base"])?;
  git(root, &["switch", "-c", "feature"])?;
  write(change)?;
  git(root, &["add", "-A"])?;
  git(root, &["commit", "-m", "change"])?;
  Ok(dir)
}

#[test]
fn prepass_astgrep_production_run_flags_a_changed_todo_macro_only() {
  let dir = repo(
    "fn old() {\n    todo!()\n}\n",
    "fn old() {\n    todo!()\n}\nfn fresh() {\n    unimplemented!()\n}\n",
  )
  .expect("repo");
  let shaped = shape(&env(), dir.path(), Some("main")).expect("shape");
  let result: PrepassResult = run(&env(), dir.path(), &shaped);
  assert_eq!(result.notices, Vec::<Notice>::new());
  assert!(!result.overflow);
  assert_eq!(result.findings.len(), 1, "{:?}", result.findings);
  let found: &Finding = &result.findings[0];
  assert_eq!(
    (found.source, found.severity, found.side),
    (Source::AstGrep, Severity::Medium, Side::New)
  );
  assert_eq!(found.category, "review-todo-macro");
  assert_eq!(
    (found.path.as_str(), found.line, found.end_line),
    ("src/lib.rs", 5, 5)
  );
  assert_eq!(found.claim, "remove todo!/unimplemented! before merge");
  assert_eq!(found.quote, "    unimplemented!()");
  assert_eq!(found.id, "prepass:ast-grep:src/lib.rs:5:review-todo-macro");
  assert_eq!(found.redaction_spans.len(), 0);
}

#[test]
fn prepass_astgrep_a_changed_todo_is_found_alongside_a_secret() {
  let secret = format!("ghp_{}", "Ab1".repeat(12));
  let dir = repo(
    "fn a() {}\n",
    &format!("fn a() {{\n    todo!()\n}}\nconst T: &str = \"{secret}\";\n"),
  )
  .expect("repo");
  let shaped = shape(&env(), dir.path(), Some("main")).expect("shape");
  let result = run(&env(), dir.path(), &shaped);
  let sources: Vec<Source> = result.findings.iter().map(|f| f.source).collect();
  assert_eq!(sources, [Source::Secrets, Source::AstGrep], "secrets first");
  let span: RedactionSpan = result.findings[0].redaction_spans[0];
  assert!(span.end > span.start);
  let doc = serde_json::to_value(&result.findings[0]).expect("json");
  assert_eq!(doc["source"], "secrets");
  assert_eq!(doc["quote"], "const T: &str = \"***\";");
  assert_eq!(doc["redaction_spans"][0]["start"], span.start);
  let lines = mechanical_checks_lines(&result);
  assert_eq!(lines.len(), 2);
  assert!(lines[0].starts_with("[secrets/blocker] src/lib.rs:4 "));
}

#[test]
fn prepass_astgrep_missing_never_reads_as_a_clean_structural_pass() {
  let dir = repo("fn a() {}\n", "fn a() {}\nfn b() {}\n").expect("repo");
  let shaped = shape(&env(), dir.path(), Some("main")).expect("shape");
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| AstGrepScan::Missing);
  let kinds: Vec<NoticeKind> = result.notices.iter().map(|notice| notice.kind).collect();
  assert_eq!(kinds, [NoticeKind::MissingAstGrep]);
  assert_eq!(
    mechanical_checks_lines(&result),
    [result.notices[0].text.clone()]
  );
}

#[test]
fn prepass_cap_is_fifty_and_a_finding_serializes_without_empty_spans() {
  assert_eq!(PREPASS_FINDINGS_MAX, 50);
  let dir = repo("fn a() {}\n", "fn a() {\n    todo!()\n}\n").expect("repo");
  let shaped = shape(&env(), dir.path(), Some("main")).expect("shape");
  let result = run(&env(), dir.path(), &shaped);
  let doc = serde_json::to_value(&result.findings[0]).expect("json");
  assert_eq!(doc["source"], "ast-grep");
  assert!(doc.get("redaction_spans").is_none());
  for key in ["path", "category", "claim", "quote"] {
    assert!(
      doc[key].as_str().is_some_and(|text| !text.is_empty()),
      "{key}"
    );
  }
}
