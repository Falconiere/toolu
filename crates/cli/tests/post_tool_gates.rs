//! `toolu hook post-tools` with its built-ins (#423): the real binary over a
//! real git project records the quality gate only from an observable exit
//! status, prints TypeScript's advisory, and passes a gate-file warning to stderr.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The `toolu` binary under test.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// A git `project` and a `home` under one temporary root.
struct Sandbox {
  _dir: tempfile::TempDir,
  root: PathBuf,
}

impl Sandbox {
  fn new() -> Res<Sandbox> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?;
    std::fs::create_dir_all(root.join("home"))?;
    let project = root.join("project");
    std::fs::create_dir_all(&project)?;
    let git = Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()?;
    if !git.success() {
      return Err("git init failed".into());
    }
    Ok(Sandbox { _dir: dir, root })
  }

  fn gate(&self) -> PathBuf {
    self
      .root
      .join("project/.claude/tmp/quality-gate-status.json")
  }

  /// `toolu hook post-tools` for a Bash call of `command` that exited `code`.
  fn ran(&self, command: &str, code: i64) -> Res<Output> {
    let stdin = serde_json::json!({
      "session_id": "s1",
      "hook_event_name": "PostToolUse",
      "tool_name": "Bash",
      "tool_input": {"command": command},
      "tool_response": {"metadata": {"exit_code": code}, "stdout": "", "stderr": ""},
    });
    Ok(
      assert_cmd::Command::new(TOOLU)
        .args(["hook", "post-tools", "--event", "PostToolUse"])
        .env_clear()
        .env("PATH", std::env::var("PATH")?)
        .env("HOME", self.root.join("home"))
        .env("CLAUDE_PROJECT_DIR", self.root.join("project"))
        .current_dir(self.root.join("project"))
        .write_stdin(stdin.to_string())
        .output()?,
    )
  }

  /// The gate file's `status`, `None` when there is no file.
  fn status(&self) -> Res<Option<String>> {
    status_of(&self.gate())
  }
}

fn status_of(gate: &Path) -> Res<Option<String>> {
  if !gate.exists() {
    return Ok(None);
  }
  let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(gate)?)?;
  Ok(
    doc
      .get("status")
      .and_then(serde_json::Value::as_str)
      .map(str::to_owned),
  )
}

/// Runs `calls` in order and returns the gate status they leave.
fn after(calls: &[(&str, i64)]) -> Res<Option<String>> {
  let sb = Sandbox::new()?;
  for (command, code) in calls {
    let out = sb.ran(command, *code)?;
    if out.status.code() != Some(0) {
      return Err(format!("{command}: {out:?}").into());
    }
  }
  sb.status()
}

const ADVISORY: &str = "{\n  \"hookSpecificOutput\": {\n    \"hookEventName\": \"PostToolUse\",\n    \"additionalContext\": \"Global quality gate failing. Fix all errors/warnings/tests before new tasks.\\\\nFailed: bun test (exit 1)\"\n  }\n}\n";

#[test]
fn a_piped_quality_command_never_records_a_pass() {
  assert_eq!(after(&[("cargo test 2>&1 | tail -5", 0)]).unwrap(), None);
  for command in [
    "cargo test 2>&1 | tail -5",
    "bun test || true",
    "bun test 2>&1 | tail -20",
  ] {
    assert_eq!(
      after(&[("bun test", 1), (command, 0)]).unwrap().as_deref(),
      Some("failing"),
      "{command}"
    );
  }
}

#[test]
fn a_proven_pass_clears_the_failure_or_writes_the_first_pass() {
  assert_eq!(
    after(&[("bun test", 1), ("cargo test", 0)])
      .unwrap()
      .as_deref(),
    Some("passing")
  );
  let sb = Sandbox::new().unwrap();
  let out = sb.ran("cargo test", 0).unwrap();
  assert_eq!((out.stdout.len(), out.status.code()), (0, Some(0)));
  let text = std::fs::read_to_string(sb.gate()).unwrap();
  assert!(
    text.starts_with(
      "{\n  \"status\": \"passing\",\n  \"source\": \"cargo test\",\n  \"updatedAt\": \""
    ),
    "{text}"
  );
}

#[test]
fn a_failing_quality_command_prints_typescripts_advisory() {
  let sb = Sandbox::new().unwrap();
  let out = sb.ran("bun test", 1).unwrap();
  assert_eq!(
    (
      String::from_utf8_lossy(&out.stdout).as_ref(),
      out.stderr.as_slice(),
      out.status.code()
    ),
    (ADVISORY, b"".as_slice(), Some(0))
  );
  assert_eq!(sb.status().unwrap().as_deref(), Some("failing"));
}

#[test]
fn an_unrecognized_gate_file_warns_on_stderr_and_is_replaced() {
  let sb = Sandbox::new().unwrap();
  std::fs::create_dir_all(sb.gate().parent().unwrap()).unwrap();
  std::fs::write(sb.gate(), "[]\n").unwrap();
  let out = sb.ran("bun test", 1).unwrap();
  assert_eq!(String::from_utf8_lossy(&out.stdout), ADVISORY);
  assert_eq!(
    String::from_utf8_lossy(&out.stderr),
    format!(
      "gate-file: unrecognized gate file at {} ((root): Invalid input: expected object, received array); replacing it\n",
      sb.gate().display()
    )
  );
  assert_eq!(sb.status().unwrap().as_deref(), Some("failing"));
}
