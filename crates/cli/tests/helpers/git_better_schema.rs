//! `toolu git-better check` from the real binary: exit 0 or 1 with one
//! `$defs/gitBetterCheck` document, stdin through `-F -`, and a usage error
//! when there is nothing to check.

use std::io::Write as _;
use std::process::{Output, Stdio};

use serde_json::{Value, json};

use crate::cli::{Res, TOOLU, one_document};
use crate::schema::violations;

/// `toolu git-better check <args>` in a private home, `stdin` piped in.
fn check(args: &[&str], stdin: &str) -> Res<Output> {
  let home = tempfile::tempdir()?;
  let mut child = std::process::Command::new(TOOLU)
    .args(["git-better", "check"])
    .args(args)
    .current_dir(home.path())
    .env_clear()
    .env("HOME", home.path())
    .env("PATH", std::env::var("PATH")?)
    .env("TOOLU_CONFIG_DIR", home.path().join("config"))
    .env("TOOLU_SETTINGS_DIR", home.path().join("settings"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  child
    .stdin
    .take()
    .ok_or("no stdin")?
    .write_all(stdin.as_bytes())?;
  Ok(child.wait_with_output()?)
}

fn document(args: &[&str], stdin: &str, code: i32) -> Res<Value> {
  let output = check(args, stdin)?;
  if output.status.code() != Some(code) {
    return Err(format!("{args:?}: expected exit {code}, got {output:?}").into());
  }
  let document = one_document(&output)?;
  let found = violations(&document, Some("gitBetterCheck"))?;
  if !found.is_empty() {
    return Err(format!("{document}: {found:?}").into());
  }
  Ok(document)
}

#[test]
fn a_failing_message_and_branch_are_one_valid_document_at_exit_1() {
  let found = document(
    &["--json", "-m", "fix stuff", "--branch", "My_Branch"],
    "",
    1,
  )
  .unwrap();
  assert_eq!(found["ok"], false);
  let targets: Vec<&Value> = found["findings"]
    .as_array()
    .unwrap()
    .iter()
    .map(|finding| &finding["target"])
    .collect();
  assert_eq!(targets, [&json!("message"), &json!("branch")]);
  assert_eq!(found["findings"][1]["example"], "<type>/my-branch");
}

#[test]
fn a_semantic_message_from_stdin_passes_at_exit_0() {
  let passed = document(
    &["--json", "-F", "-", "--branch", "feat/740-git-better"],
    "feat(git-better): add check\n\nIntent: validate before committing\n",
    0,
  )
  .unwrap();
  assert_eq!(passed["ok"], true);
  assert_eq!(passed["findings"], json!([]));
  let plain = check(&["-m", "fix(x): y", "-m", "Intent: z"], "").unwrap();
  assert_eq!(plain.status.code(), Some(0));
  assert_eq!(String::from_utf8(plain.stdout).unwrap(), "semantic: ok\n");
}

#[test]
fn nothing_to_check_is_a_usage_error_and_a_bad_target_is_refused() {
  let usage = check(&[], "").unwrap();
  assert_eq!(usage.status.code(), Some(64));
  let mut found = document(&["--json", "--branch", "x"], "", 1).unwrap();
  found["findings"][0]["target"] = json!("tag");
  assert_ne!(
    violations(&found, Some("gitBetterCheck")).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn setup_adds_the_rewrite_ref_once() {
  let home = tempfile::tempdir().unwrap();
  let repo = home.path().join("repo");
  std::fs::create_dir_all(&repo).unwrap();
  let init = std::process::Command::new("git")
    .args(["init", "-q"])
    .arg(&repo)
    .status()
    .unwrap();
  assert!(init.success());
  let setup = || {
    let output = std::process::Command::new(TOOLU)
      .args(["--json", "git-better", "setup"])
      .current_dir(&repo)
      .env("HOME", home.path())
      .env("TOOLU_CONFIG_DIR", home.path().join("config"))
      .output()
      .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document = one_document(&output).unwrap();
    assert_eq!(
      violations(&document, Some("gitBetterSetup")).unwrap(),
      Vec::<String>::new()
    );
    document["changed"].clone()
  };
  assert_eq!(setup(), json!(true));
  assert_eq!(setup(), json!(false));
  let refs = std::process::Command::new("git")
    .args(["config", "--get-all", "notes.rewriteRef"])
    .current_dir(&repo)
    .output()
    .unwrap();
  assert_eq!(
    String::from_utf8(refs.stdout).unwrap(),
    "refs/notes/git-better\n"
  );
}
