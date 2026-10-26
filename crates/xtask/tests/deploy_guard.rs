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
  child
    .stdin
    .take()
    .ok_or("no stdin pipe")?
    .write_all(stdin.as_bytes())
    .map_err(|err| err.to_string())?;
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
