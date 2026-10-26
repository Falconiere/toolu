//! `cargo xtask deploy-guard` through the real binary, as the `/deploy` skill's
//! `PreToolUse` hook runs it: the payload on stdin, exit 2 to block.

use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Run `deploy-guard` against a fresh root with `stdin` as the hook payload.
fn guard(stdin: &str) -> Result<Output, String> {
  let root = tempfile::tempdir().map_err(|err| err.to_string())?;
  let mut child = Command::new(env!("CARGO_BIN_EXE_xtask"))
    .args(["deploy-guard", "--root"])
    .arg(root.path())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| err.to_string())?;
  feed(&mut child, stdin)?;
  child.wait_with_output().map_err(|err| err.to_string())
}

#[test]
fn an_ordinary_command_exits_0_with_no_output() {
  let output =
    guard(r#"{"tool_name":"Bash","tool_input":{"command":"git status --porcelain"}}"#).unwrap();
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  assert!(
    output.stdout.is_empty() && output.stderr.is_empty(),
    "{output:?}"
  );
}

#[test]
fn invalid_empty_or_commandless_stdin_exits_2() {
  for stdin in ["not json", "", r#"{"tool_input":{}}"#] {
    let output = guard(stdin).unwrap();
    assert_eq!(output.status.code(), Some(2), "{stdin:?}: {output:?}");
    assert!(
      String::from_utf8_lossy(&output.stderr).contains("deploy-guard:"),
      "{output:?}"
    );
  }
}

#[test]
fn a_tag_push_without_a_receipt_exits_2_naming_the_receipt() {
  let output = guard(r#"{"tool_input":{"command":"git push origin refs/tags/v9.9.9"}}"#).unwrap();
  assert_eq!(output.status.code(), Some(2), "{output:?}");
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("no gate receipt at target/deploy/gate.json"),
    "{output:?}"
  );
}

/// The skill's hook command, from the real frontmatter.
fn hook_command() -> Result<String, String> {
  let skill = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../.claude/skills/deploy/SKILL.md"
  ))
  .map_err(|err| err.to_string())?;
  let (_, rest) = skill.split_once("---\n").ok_or("no frontmatter")?;
  let (yaml, _) = rest.split_once("\n---\n").ok_or("no frontmatter end")?;
  let matter: serde_json::Value = serde_saphyr::from_str(yaml).map_err(|err| err.to_string())?;
  matter
    .pointer("/hooks/PreToolUse/0/hooks/0/command")
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
    .ok_or_else(|| "no hook command".to_owned())
}

/// Run the hook command for a project at `project` with no cargo on `PATH`, as
/// on a checkout where `cargo xtask deploy-guard` cannot run.
fn hook(project: &std::path::Path, stdin: &str) -> Result<Output, String> {
  let mut command = Command::new("/bin/sh");
  command.args(["-c", &hook_command()?]).env_clear();
  // The snapshot is the instrumented xtask under coverage: keep its profiles out of the crate.
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let mut child = command
    .env("PATH", "/usr/bin:/bin")
    .env("HOME", project)
    .env("CLAUDE_PROJECT_DIR", project)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| err.to_string())?;
  feed(&mut child, stdin)?;
  child.wait_with_output().map_err(|err| err.to_string())
}

#[test]
fn the_hook_runs_the_snapshot_without_cargo_and_fails_closed_without_either() {
  let project = tempfile::tempdir().unwrap();
  let tag_push =
    r#"{"tool_name":"Bash","tool_input":{"command":"git push origin refs/tags/v9.9.9"}}"#;
  assert_ne!(
    hook(project.path(), tag_push).unwrap().status.code(),
    Some(0)
  );
  let snapshot = project.path().join(".claude/tmp/deploy-guard");
  std::fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
  std::fs::copy(env!("CARGO_BIN_EXE_xtask"), &snapshot).unwrap();
  for allowed in [
    r#"{"tool_name":"Bash","tool_input":{"command":"git switch main"}}"#,
    r#"{"tool_name":"Edit","permission_mode":"acceptEdits","tool_input":{"file_path":"/w/src/lib.rs"}}"#,
  ] {
    let output = hook(project.path(), allowed).unwrap();
    assert_eq!(output.status.code(), Some(0), "{allowed}: {output:?}");
  }
  let output = hook(project.path(), tag_push).unwrap();
  assert_eq!(output.status.code(), Some(2), "{output:?}");
  assert!(
    String::from_utf8_lossy(&output.stderr).contains("no gate receipt"),
    "{output:?}"
  );
}

/// Write `stdin` to the child and close it. A child that exits before reading
/// (the fail-closed hook) may close the pipe first; that is not a failure.
fn feed(child: &mut std::process::Child, stdin: &str) -> Result<(), String> {
  let mut pipe = child.stdin.take().ok_or("no stdin pipe")?;
  match pipe.write_all(stdin.as_bytes()) {
    Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => Err(err.to_string()),
    _ => Ok(()),
  }
}
