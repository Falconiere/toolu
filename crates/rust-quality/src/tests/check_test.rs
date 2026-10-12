use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{Report, check, render};

/// A git repository holding `files`, checked with the real `PATH` and a home of its own.
fn repo(files: &[(&str, String)]) -> (tempfile::TempDir, Env) {
  let dir = tempfile::tempdir().unwrap();
  for (path, body) in files {
    let at = dir.path().join(path);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, body).unwrap();
  }
  assert!(
    Command::new("git")
      .arg("-C")
      .arg(dir.path())
      .args(["init", "-q"])
      .status()
      .unwrap()
      .success()
  );
  let home = dir.path().join("home").display().to_string();
  let env = Env::process()
    .with("HOME", &home)
    .with("TOOLU_CONFIG_DIR", &home);
  (dir, env)
}

fn over_limit() -> String {
  (0..320)
    .map(|at| format!("const A{at}: u32 = {at};"))
    .collect::<Vec<_>>()
    .join("\n")
}

const CAPPED: &str = r#"{"lang":{"rust":{"maxFileLines":300}}}"#;

#[test]
fn a_module_over_the_configured_limit_is_a_finding() {
  let (dir, env) = repo(&[
    ("crates/demo/src/over.rs", over_limit()),
    ("crates/demo/src/fits.rs", "//! Fits.\n".to_owned()),
    (".claude/toolu.config.json", CAPPED.to_owned()),
    (".gitignore", "target/\n".to_owned()),
    (
      "target/debug/build.rs",
      "fn main() { None::<u8>.unwrap(); }\n".to_owned(),
    ),
  ]);
  let report = check(&env, Host::Claude, dir.path(), &["crates".to_owned()]).unwrap();
  assert_eq!(report.files, 2, "ignored files are not listed");
  assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
  let (path, message) = &report.findings[0];
  assert_eq!(path, "crates/demo/src/over.rs");
  assert!(
    message.starts_with("File exceeds 300-line limit: crates/demo/src/over.rs (320 code lines"),
    "{message}"
  );
  let from_inside = check(&env, Host::Claude, &dir.path().join("crates/demo"), &[]).unwrap();
  assert_eq!(from_inside, report, "the default is the whole repository");
  let text = render(&report, false);
  assert_eq!(
    (text.exit, text.stderr.as_deref()),
    (Exit::Failure, Some("rust-quality: 1 finding(s)"))
  );
  assert_eq!(text.stdout.as_deref(), Some(message.as_str()));
}

#[test]
fn without_config_the_default_limit_lets_it_pass() {
  let (dir, env) = repo(&[("crates/demo/src/over.rs", over_limit())]);
  let report = check(&env, Host::Claude, dir.path(), &[]).unwrap();
  assert_eq!(
    report,
    Report {
      files: 1,
      findings: Vec::new()
    }
  );
  let text = render(&report, false);
  assert_eq!(
    (text.exit, text.stdout, text.stderr.as_deref()),
    (Exit::Success, None, Some("rust-quality: 1 files, ok"))
  );
  let doc: Value = serde_json::from_str(&render(&report, true).stdout.unwrap()).unwrap();
  assert_eq!(doc, serde_json::json!({"files": 1, "findings": []}));
}

#[test]
fn a_top_level_src_gets_the_src_rules_and_scans_are_shared_per_chunk() {
  let mut files: Vec<(String, String)> = (0..70)
    .map(|at| {
      (
        format!("src/m{at:02}.rs"),
        format!("//! M.\nfn f() {{ let _ = Some({at}).unwrap(); }}\n"),
      )
    })
    .collect();
  files.push((
    "src/tests/a_test.rs".to_owned(),
    "use mockall::predicate;\n".to_owned(),
  ));
  let borrowed: Vec<(&str, String)> = files
    .iter()
    .map(|(path, body)| (path.as_str(), body.clone()))
    .collect();
  let (dir, env) = repo(&borrowed);
  let report = check(&env, Host::Claude, dir.path(), &[]).unwrap();
  assert_eq!(
    (report.files, report.findings.len()),
    (71, 71),
    "{:?}",
    report.findings
  );
  let (path, message) = &report.findings[69];
  assert_eq!(path, "src/m69.rs");
  assert_eq!(
    message,
    ".unwrap() in src/m69.rs — use ? or match on Result/Option\nsrc/m69.rs:2:fn f() { let _ = Some(69).unwrap(); }"
  );
  let (path, message) = &report.findings[70];
  assert_eq!(path, "src/tests/a_test.rs");
  assert!(
    message.starts_with("no-mocks: mockall/faux import in src/tests/a_test.rs"),
    "{message}"
  );
  let doc: Value = serde_json::from_str(&render(&report, true).stdout.unwrap()).unwrap();
  assert_eq!(doc["findings"][70]["path"], "src/tests/a_test.rs");
}

#[test]
fn an_unreadable_file_is_a_finding() {
  let (dir, env) = repo(&[("src/locked.rs", "//! L.\n".to_owned())]);
  let locked = dir.path().join("src/locked.rs");
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
  let report = check(&env, Host::Claude, dir.path(), &[]).unwrap();
  std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert_eq!(report.findings.len(), 1);
  assert!(
    report.findings[0]
      .1
      .starts_with("Cannot read src/locked.rs for Rust quality checks: ")
  );
}

#[test]
fn it_fails_closed_without_ast_grep_or_a_repository() {
  let (dir, env) = repo(&[("src/a.rs", "//! A.\n".to_owned())]);
  let bare = env.clone().with("PATH", "/usr/bin:/bin");
  let (exit, message) = check(&bare, Host::Claude, dir.path(), &[]).unwrap_err();
  assert_eq!(exit, Exit::Unavailable);
  assert_eq!(
    message,
    "rust-quality: ast-grep is not installed (npm install -g @ast-grep/cli)"
  );
  let outside = tempfile::tempdir().unwrap();
  let (exit, message) = check(&env, Host::Claude, outside.path(), &[]).unwrap_err();
  assert_eq!(exit, Exit::Failure);
  assert!(
    message.ends_with("is not inside a git repository"),
    "{message}"
  );
  let empty = check(&env, Host::Claude, dir.path(), &["missing/".to_owned()]).unwrap();
  assert_eq!(empty, Report::default(), "a path with no files is clean");
}

#[test]
fn only_files_under_the_paths_that_still_exist_are_checked() {
  let (dir, env) = repo(&[
    ("crates/demo/src/kept.rs", "//! Kept.\n".to_owned()),
    ("crates/demo/src/gone.rs", over_limit()),
    ("tools/outside.rs", over_limit()),
    (".claude/toolu.config.json", CAPPED.to_owned()),
  ]);
  let git = |args: &[&str]| {
    let status = Command::new("git")
      .args(["-c", "user.name=t", "-c", "user.email=t@t", "-C"])
      .arg(dir.path())
      .args(args)
      .status()
      .unwrap();
    assert!(status.success(), "git {args:?}");
  };
  git(&["add", "-A"]);
  git(&["commit", "-q", "-m", "init"]);
  std::fs::remove_file(dir.path().join("crates/demo/src/gone.rs")).unwrap();
  let report = check(&env, Host::Claude, dir.path(), &["crates".to_owned()]).unwrap();
  assert_eq!(
    report,
    Report {
      files: 1,
      findings: Vec::new()
    }
  );
}
