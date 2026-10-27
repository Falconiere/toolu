//! End-to-end documentation review through the native binary and real Git.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn std::error::Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

struct Fixture {
  temp: tempfile::TempDir,
  repo: PathBuf,
}

impl Fixture {
  fn new() -> Res<Self> {
    let temp = tempfile::tempdir()?;
    // Git reports the resolved toplevel; a symlinked TMPDIR must not change the prefix.
    let repo = temp.path().canonicalize()?.join("repo");
    fs::create_dir_all(&repo)?;
    let fixture = Self { temp, repo };
    fixture.git(&["init", "-q", "-b", "main"])?;
    fixture.git(&["config", "user.name", "Test"])?;
    fixture.git(&["config", "user.email", "test@example.com"])?;
    fs::write(fixture.repo.join("README.md"), "Initial guide\n")?;
    fixture.commit("base")?;
    fixture.git(&["checkout", "-q", "-b", "feature"])?;
    fs::write(fixture.repo.join("code.rs"), "new behavior\n")?;
    fs::write(fixture.repo.join("README.md"), "Unrelated edit\n")?;
    fixture.commit("change")?;
    Ok(fixture)
  }

  fn git(&self, args: &[&str]) -> Res<()> {
    let output = Command::new("git")
      .env_clear()
      .env("PATH", std::env::var_os("PATH").ok_or("PATH missing")?)
      .env("HOME", self.temp.path())
      .current_dir(&self.repo)
      .args(args)
      .output()?;
    if !output.status.success() {
      return Err(format!("git failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(())
  }

  fn commit(&self, message: &str) -> Res<()> {
    self.git(&["add", "."])?;
    self.git(&["commit", "-q", "-m", message])
  }

  fn command(&self, args: &[&str], stdin: Option<&str>) -> Res<Output> {
    let mut command = assert_cmd::Command::new(TOOLU);
    command
      .args(args)
      .current_dir(&self.repo)
      .env_clear()
      .env("PATH", std::env::var_os("PATH").ok_or("PATH missing")?)
      .env("HOME", self.temp.path())
      .env("TOOLU_CONFIG_DIR", self.temp.path().join("config"))
      .env("TOOLU_PROJECT_DIR", &self.repo)
      .env("TOOLU_HOST_OVERRIDE", "claude")
      .env("DOCS_SYNC_BASE", "main");
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    if let Some(input) = stdin {
      command.write_stdin(input);
    }
    Ok(command.output()?)
  }

  fn inspect(&self) -> Res<Value> {
    let output = self.command(&["docs", "inspect"], None)?;
    if !output.status.success() {
      return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
  }

  fn report(&self) -> Res<Value> {
    let mut report = self.inspect()?;
    let fields = report
      .as_object_mut()
      .ok_or("inspection is not an object")?;
    fields.insert("route".into(), json!({"source":"jev","candidate_id":"review","host":"codex","model":"configured","effort":"high"}));
    fields.insert(
      "worker".into(),
      json!({"id":"worker-1","host":"codex","model":"configured","effort":"high"}),
    );
    fields.insert("coverage".into(), json!([
      {"path":"README.md","disposition":"not-needed","note":"Unrelated edit reviewed","docs":[]},
      {"path":"code.rs","disposition":"accurate","note":"Guide describes the behavior","docs":["README.md"]}
    ]));
    fields.insert(
      "checks".into(),
      json!(["Checked guide against implementation"]),
    );
    fields.insert("findings".into(), json!([]));
    Ok(report)
  }

  fn write(&self, report: &Value) -> Res<Output> {
    let path = self.temp.path().join("report.json");
    fs::write(&path, serde_json::to_vec(report)?)?;
    self.command(
      &[
        "docs",
        "write-state",
        "--host",
        "claude",
        "--report",
        path.to_str().ok_or("report path")?,
      ],
      None,
    )
  }

  /// `toolu` with no host override or project dir pointing at the repository.
  fn bare(&self, args: &[&str]) -> Res<Output> {
    let mut command = assert_cmd::Command::new(TOOLU);
    command
      .args(args)
      .current_dir(&self.repo)
      .env_clear()
      .env("PATH", std::env::var_os("PATH").ok_or("PATH missing")?)
      .env("HOME", self.temp.path())
      .env("TOOLU_PROJECT_DIR", self.temp.path())
      .env("DOCS_SYNC_BASE", "main");
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }

  fn register(&self) -> Res<Output> {
    self.command(
      &[
        "documentation-gate",
        "hook",
        "register",
        "--event",
        "SessionStart",
      ],
      Some("{}"),
    )
  }

  fn push(&self) -> Res<Output> {
    let input = json!({"tool_name":"Bash","tool_input":{"command":"git push origin feature"},"session_id":"test"});
    self.command(
      &["hook", "pre-tools", "--event", "PreToolUse"],
      Some(&input.to_string()),
    )
  }
}

fn reason(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn native_gate_requires_current_complete_review() {
  let fixture = Fixture::new().unwrap();
  assert!(fixture.register().unwrap().status.success());
  let manifest = fixture
    .temp
    .path()
    .join("config/toolu/pre-tools.d/documentation-gate@toolu__docs-review.json");
  assert!(manifest.is_file());
  let before = fixture.push().unwrap();
  assert!(
    reason(&before).contains("documentation-gate"),
    "{}",
    reason(&before)
  );
  let report = fixture.report().unwrap();
  let mut partial = report.clone();
  partial
    .as_object_mut()
    .unwrap()
    .insert("coverage".into(), json!([]));
  assert!(!fixture.write(&partial).unwrap().status.success());
  let path = fixture.temp.path().join("report.json");
  fs::write(&path, report.to_string()).unwrap();
  // No host flag or marker and a project dir elsewhere: the repository is the
  // current directory, and every host's gate reads the review.
  let written = fixture
    .bare(&["docs", "write-state", "--report", path.to_str().unwrap()])
    .unwrap();
  assert!(
    written.status.success(),
    "{}",
    String::from_utf8_lossy(&written.stderr)
  );
  let path: Value = serde_json::from_slice(&written.stdout).unwrap();
  let state = Path::new(path["path"].as_str().unwrap());
  assert!(state.is_file());
  assert!(state.starts_with(fixture.repo.join(".claude/tmp/docs-review")));
  assert!(!reason(&fixture.push().unwrap()).contains("documentation-gate"));
  fs::write(fixture.repo.join("code.rs"), "newer behavior\n").unwrap();
  fixture.commit("new change").unwrap();
  assert!(!fixture.write(&report).unwrap().status.success());
  assert!(reason(&fixture.push().unwrap()).contains("documentation-gate"));
}

#[test]
fn route_rejects_bad_candidates_and_does_not_read_dotenv() {
  let fixture = Fixture::new().unwrap();
  let candidates = fixture.temp.path().join("candidates.json");
  let evidence = fixture.temp.path().join("evidence.txt");
  fs::write(&evidence, "Documentation review needs a worker").unwrap();
  fs::write(fixture.repo.join(".env"), "TYPESAFE_API_KEY=not-exported\n").unwrap();
  let args = [
    "docs",
    "route",
    "--candidates",
    candidates.to_str().unwrap(),
    "--evidence",
    evidence.to_str().unwrap(),
  ];
  fs::write(&candidates, "[]").unwrap();
  let invalid = fixture.command(&args, None).unwrap();
  assert!(!invalid.status.success());
  assert!(String::from_utf8_lossy(&invalid.stderr).contains("candidates"));
  fs::write(&candidates, json!([{"id":"worker","host":"codex","model":"configured","effort":"high","description":"ready"}]).to_string()).unwrap();
  let missing = fixture.command(&args, None).unwrap();
  assert!(!missing.status.success());
  assert!(String::from_utf8_lossy(&missing.stderr).contains("TYPESAFE_API_KEY"));
}
