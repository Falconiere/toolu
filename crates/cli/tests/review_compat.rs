//! A v2 document produced by the former Bun writer is read by the native push gate.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn git(dir: &Path, args: &[&str]) -> Res<()> {
  let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
  if !out.status.success() {
    return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
  }
  Ok(())
}

fn repo(dir: &Path) -> Res<()> {
  fs::create_dir_all(dir)?;
  git(dir, &["init", "-q", "-b", "main"])?;
  git(dir, &["config", "user.email", "test@example.com"])?;
  git(dir, &["config", "user.name", "Test"])?;
  fs::write(dir.join("base.txt"), "base\n")?;
  git(dir, &["add", "base.txt"])?;
  git(dir, &["commit", "-qm", "base"])?;
  git(dir, &["checkout", "-qb", "feature"])?;
  fs::write(dir.join("feature.txt"), "reviewed change\n")?;
  git(dir, &["add", "feature.txt"])?;
  git(dir, &["commit", "-qm", "feature"])?;
  Ok(())
}

fn gate(dir: &Path) -> Res<Output> {
  let payload = json!({"tool_name":"Bash","tool_input":{"command":"git push origin feature"}});
  Ok(
    assert_cmd::Command::new(TOOLU)
      .args(["hook", "pre-tools", "--event", "PreToolUse"])
      .env("TOOLU_HOST_OVERRIDE", "codex")
      .env("PUSH_REVIEW_BASE", "main")
      .current_dir(dir)
      .write_stdin(payload.to_string())
      .output()?,
  )
}

fn writer(dir: &Path, args: &[&str]) -> Res<Output> {
  writer_host(dir, "codex", args)
}

fn writer_host(dir: &Path, host: &str, args: &[&str]) -> Res<Output> {
  Ok(
    assert_cmd::Command::new(TOOLU)
      .args(["review", "write-state"])
      .args(args)
      .env("TOOLU_HOST_OVERRIDE", host)
      .env("PUSH_REVIEW_BASE", "main")
      .current_dir(dir)
      .output()?,
  )
}

#[test]
fn native_writer_uses_each_hosts_project_state_directory() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  for host in ["claude", "codex", "opencode"] {
    let output = writer_host(&dir, host, &["--findings-count", "0"]).unwrap();
    assert_eq!(
      output.status.code(),
      Some(0),
      "{host}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    let file = dir.join(format!(".{host}/tmp/push-review/feature.json"));
    assert_eq!(
      String::from_utf8_lossy(&output.stdout).trim(),
      file.canonicalize().unwrap().display().to_string()
    );
    let doc: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    assert_eq!(doc["version"], 2, "{host}");
  }
}

#[test]
fn native_writer_json_reports_the_written_file() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  let output = writer(&dir, &["--findings-count", "0", "--json"]).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
  let file = dir.join(".codex/tmp/push-review/feature.json");
  assert_eq!(
    reply,
    json!({"path":file.canonicalize().unwrap().display().to_string()})
  );
  assert!(file.is_file());
}

fn state(dir: &Path) -> Res<Value> {
  let file = dir.join(".codex/tmp/push-review/feature.json");
  Ok(serde_json::from_slice(&fs::read(file)?)?)
}

#[test]
fn legacy_writer_state_allows_the_reviewed_diff_then_denies_a_new_commit() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  let config_dir = dir.join(".codex");
  fs::create_dir_all(config_dir.join("tmp/push-review")).unwrap();
  fs::write(
    config_dir.join("toolu.config.json"),
    r#"{"version":1,"gates":{"preset":"strict"}}"#,
  )
  .unwrap();
  let legacy = include_str!("fixtures/review_legacy.json");
  fs::write(config_dir.join("tmp/push-review/feature.json"), legacy).unwrap();
  let accepted = gate(&dir).unwrap();
  assert_eq!(accepted.status.code(), Some(0));
  assert!(
    accepted.stdout.is_empty(),
    "{}",
    String::from_utf8_lossy(&accepted.stdout)
  );
  fs::write(dir.join("feature.txt"), "reviewed change\nnew commit\n").unwrap();
  git(&dir, &["add", "feature.txt"]).unwrap();
  git(&dir, &["commit", "-qm", "new commit"]).unwrap();
  let stale = gate(&dir).unwrap();
  let doc: Value = serde_json::from_slice(&stale.stdout).unwrap();
  assert_eq!(
    doc.pointer("/hookSpecificOutput/permissionDecision"),
    Some(&json!("deny")),
    "{doc}"
  );
}

#[test]
fn native_writer_allows_the_reviewed_diff_and_resets_after_a_commit() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  fs::create_dir_all(dir.join(".codex")).unwrap();
  fs::write(
    dir.join(".codex/toolu.config.json"),
    r#"{"version":1,"gates":{"preset":"strict"}}"#,
  )
  .unwrap();
  let args = [
    "--findings-count",
    "0",
    "--reviewers",
    "[\"toolu-review:review\"]",
  ];
  let output = writer(&dir, &args).unwrap();
  assert_eq!(
    output.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let file = dir.join(".codex/tmp/push-review/feature.json");
  assert_eq!(
    String::from_utf8_lossy(&output.stdout).trim(),
    file.canonicalize().unwrap().display().to_string()
  );
  let first = state(&dir).unwrap();
  let legacy: Value = serde_json::from_str(include_str!("fixtures/review_legacy.json")).unwrap();
  assert_eq!(first["diff_sha"], legacy["diff_sha"]);
  assert_eq!(first["reviewed_files"], legacy["reviewed_files"]);
  assert_eq!(first["review_round"], 1);
  assert_eq!(gate(&dir).unwrap().stdout, Vec::<u8>::new());
  assert_eq!(writer(&dir, &args).unwrap().status.code(), Some(0));
  assert_eq!(state(&dir).unwrap()["review_round"], 2);
  fs::write(dir.join("feature.txt"), "reviewed change\nnew commit\n").unwrap();
  git(&dir, &["add", "feature.txt"]).unwrap();
  git(&dir, &["commit", "-qm", "new commit"]).unwrap();
  let denied: Value = serde_json::from_slice(&gate(&dir).unwrap().stdout).unwrap();
  assert_eq!(
    denied.pointer("/hookSpecificOutput/permissionDecision"),
    Some(&json!("deny"))
  );
  assert_eq!(writer(&dir, &args).unwrap().status.code(), Some(0));
  assert_eq!(state(&dir).unwrap()["review_round"], 1);
  assert_eq!(gate(&dir).unwrap().stdout, Vec::<u8>::new());
}

#[test]
fn native_writer_rejects_bad_json_branch_and_empty_diff() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  let bad_json = writer(&dir, &["--findings-count", "0", "--findings", "[bad"]).unwrap();
  assert_eq!(bad_json.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&bad_json.stderr).contains("bad --findings JSON"));
  let bad_branch = writer(&dir, &["--findings-count", "0", "--branch", "other"]).unwrap();
  assert_eq!(bad_branch.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&bad_branch.stderr).contains("does not match"));
  let bad_count = writer(&dir, &["--findings-count", "many"]).unwrap();
  assert_eq!(bad_count.status.code(), Some(64));
  assert!(!dir.join(".codex/tmp/push-review/feature.json").exists());
  git(&dir, &["checkout", "-q", "main"]).unwrap();
  let empty = writer(&dir, &["--findings-count", "0"]).unwrap();
  assert_eq!(empty.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&empty.stderr).contains("empty"));
}

#[test]
fn native_writer_keys_a_detached_worktree_to_the_requested_branch() {
  let tmp = tempfile::tempdir().unwrap();
  let dir = tmp.path().join("project");
  repo(&dir).unwrap();
  git(&dir, &["checkout", "-q", "main"]).unwrap();
  let worktree = tmp.path().join("detached");
  git(
    &dir,
    &[
      "worktree",
      "add",
      "-q",
      "--detach",
      worktree.to_str().unwrap(),
      "feature",
    ],
  )
  .unwrap();
  let checkout = worktree.to_str().unwrap();
  let missing = writer(&dir, &["--findings-count", "0", "--repo", checkout]).unwrap();
  assert_eq!(missing.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&missing.stderr).contains("--branch"));
  let recorded = writer(
    &dir,
    &[
      "--findings-count",
      "0",
      "--repo",
      checkout,
      "--branch",
      "feature",
    ],
  )
  .unwrap();
  assert_eq!(
    recorded.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&recorded.stderr)
  );
  let file = worktree.join(".codex/tmp/push-review/feature.json");
  assert_eq!(
    String::from_utf8_lossy(&recorded.stdout).trim(),
    file.canonicalize().unwrap().display().to_string()
  );
  let doc: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
  assert_eq!(doc["branch"], "feature");
}

#[test]
fn native_session_start_publishes_the_compatibility_shim_on_each_host() {
  let tmp = tempfile::tempdir().unwrap();
  let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu-review");
  for host in ["claude", "codex", "opencode"] {
    let config = tmp.path().join(host);
    let output = assert_cmd::Command::new(TOOLU)
      .args([
        "toolu-review",
        "hook",
        "session-start",
        "--event",
        "SessionStart",
        "--plugin-root",
        plugin.to_str().unwrap(),
      ])
      .env("TOOLU_HOST_OVERRIDE", host)
      .env("TOOLU_CONFIG_DIR", &config)
      .write_stdin(r#"{"session_id":"review-compat"}"#)
      .output()
      .unwrap();
    assert_eq!(
      output.status.code(),
      Some(0),
      "{host}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    let shim = config.join("toolu-review/write-state.sh");
    assert!(
      fs::symlink_metadata(&shim)
        .unwrap()
        .file_type()
        .is_symlink(),
      "{host}"
    );
    assert_eq!(
      fs::read_to_string(&shim).unwrap(),
      "#!/bin/sh\nexec toolu review write-state \"$@\"\n"
    );
  }
}
