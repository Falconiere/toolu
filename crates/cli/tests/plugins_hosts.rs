//! `toolu plugins` against Claude Code and Codex: the real binary with stub
//! `claude`/`codex` CLIs on `PATH` that log their argv and fail on cue, so the
//! core-failure and `--json` paths can be driven (#438, AC-3, AC-9).

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-cli-")
      .tempdir()?;
    std::fs::create_dir_all(dir.path().join("bin"))?;
    std::fs::create_dir_all(dir.path().join("home"))?;
    Ok(Sb { dir })
  }

  fn root(&self) -> PathBuf {
    self.dir.path().to_path_buf()
  }

  /// A host CLI that logs argv, lists nothing, and fails argv matching `fail`.
  fn host(&self, name: &str, empty: &str, fail: &str) -> Res<()> {
    let root = self.root();
    let log = root.join(format!("{name}.log"));
    let body = format!(
      "#!/bin/sh\necho \"$*\" >> {log}\ncase \"$*\" in\n\"plugin list --json\"|\"plugin list --available --json\") echo '{empty}' ;;\n{fail}) echo \"boom: $*\" >&2; exit 1 ;;\n*) exit 0 ;;\nesac\n",
      log = log.display()
    );
    let temp = root.join(format!("bin/.{name}"));
    std::fs::write(&temp, body)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, root.join(format!("bin/{name}")))?;
    Ok(())
  }

  fn log(&self, name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(self.root().join(format!("{name}.log"))).unwrap_or_default();
    text.lines().map(str::to_owned).collect()
  }

  fn run(&self, cwd: &Path, args: &[&str]) -> Res<Output> {
    let path = format!("{}:/usr/bin:/bin", self.root().join("bin").display());
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .current_dir(cwd)
      .env_clear()
      .env("PATH", path)
      .env("HOME", self.root().join("home"));
    // Coverage runs keep their profile path, so no `.profraw` lands in the cwd.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_core_failure_skips_its_dependents_and_jev_still_installs() {
  let sb = Sb::new().unwrap();
  sb.host("claude", "[]", "\"plugin install toolu@toolu\"")
    .unwrap();
  let out = sb
    .run(
      &sb.root(),
      &[
        "plugins",
        "install",
        "toolu",
        "ts-quality",
        "jev",
        "--host",
        "claude",
      ],
    )
    .unwrap();
  assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
  assert_eq!(
    text(&out.stdout),
    "  x toolu            boom: plugin install toolu@toolu\n  - ts-quality       skipped: toolu failed\n  + jev              installed\n"
  );
  let log = sb.log("claude");
  assert!(
    log.contains(&"plugin install jev@toolu".to_owned()),
    "{log:?}"
  );
  assert!(
    !log.iter().any(|line| line.contains("ts-quality")),
    "{log:?}"
  );
}

#[test]
fn json_is_one_document_on_success_and_on_failure() {
  let sb = Sb::new().unwrap();
  sb.host("claude", "[]", "\"plugin install toolu@toolu\"")
    .unwrap();
  let failed = sb
    .run(
      &sb.root(),
      &["plugins", "install", "toolu", "--host", "claude", "--json"],
    )
    .unwrap();
  assert_eq!(failed.status.code(), Some(1));
  let doc: Value = serde_json::from_slice(&failed.stdout).unwrap();
  assert_eq!(doc["hosts"][0]["steps"][0]["outcome"], "failed");
  assert_eq!(doc["hosts"][0]["host"], "claude");
  let listed = sb
    .run(
      &sb.root(),
      &["--json", "plugins", "list", "--host", "claude"],
    )
    .unwrap();
  assert_eq!(listed.status.code(), Some(0));
  let rows: Value = serde_json::from_slice(&listed.stdout).unwrap();
  assert!(
    rows
      .as_array()
      .is_some_and(|rows| rows.iter().all(|row| row["installed"] == false))
  );
  sb.host("codex", r#"{"installed":[]}"#, "never").unwrap();
  let removed = sb
    .run(
      &sb.root(),
      &[
        "plugins", "remove", "jev", "--yes", "--host", "codex", "--json",
      ],
    )
    .unwrap();
  let doc: Value = serde_json::from_slice(&removed.stdout).unwrap();
  assert_eq!(
    (doc["host"].clone(), doc["steps"][0]["removed"].clone()),
    (Value::from("codex"), Value::Bool(true))
  );
  let update = sb
    .run(
      &sb.root(),
      &["plugins", "update", "--host", "codex", "--json"],
    )
    .unwrap();
  assert_eq!(update.status.code(), Some(1));
  let doc: Value = serde_json::from_slice(&update.stdout).unwrap();
  assert_eq!(doc["error"]["name"], "failure", "{doc}");
}

#[test]
fn a_dry_run_works_with_no_host_cli_from_any_directory() {
  let sb = Sb::new().unwrap();
  let empty = sb.root().join("empty");
  std::fs::create_dir_all(&empty).unwrap();
  let out = sb
    .run(
      &empty,
      &[
        "plugins",
        "install",
        "--dry-run",
        "--host",
        "claude",
        "rust-quality",
      ],
    )
    .unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  assert_eq!(
    text(&out.stdout),
    "Planned commands (nothing was run):\n  claude plugin install toolu@toolu\n  claude plugin install rust-quality@toolu\n"
  );
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let inside = sb
    .run(
      &repo,
      &["plugins", "install", "--dry-run", "--host", "codex", "jev"],
    )
    .unwrap();
  assert_eq!(
    text(&inside.stdout),
    "Planned commands (nothing was run):\n  codex plugin add jev@toolu\n"
  );
}
