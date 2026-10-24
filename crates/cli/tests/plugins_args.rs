//! `toolu plugins` flags against the real binary: `--scope` per host,
//! `--config` replay, `--no-input`, host refusal, and the exit codes of the
//! CLI contract (#438, AC-4, AC-9, AC-11).

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-args-")
      .tempdir()?;
    std::fs::create_dir_all(dir.path().join("bin"))?;
    Ok(Sb { dir })
  }

  fn root(&self) -> PathBuf {
    self.dir.path().to_path_buf()
  }

  fn stub(&self, name: &str, listing: &str) -> Res<()> {
    let temp = self.root().join(format!("bin/.{name}"));
    let body = format!("#!/bin/sh\ncase \"$*\" in\n*--json) echo '{listing}' ;;\nesac\nexit 0\n");
    std::fs::write(&temp, body)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, self.root().join(format!("bin/{name}")))?;
    Ok(())
  }

  fn write(&self, rel: &str, text: &str) -> Res<PathBuf> {
    let path = self.root().join(rel);
    std::fs::write(&path, text)?;
    Ok(path)
  }

  fn run(&self, args: &[&str]) -> Res<Output> {
    let path = format!("{}:/usr/bin:/bin", self.root().join("bin").display());
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .current_dir(self.root())
      .env_clear()
      .env("PATH", path)
      .env("HOME", self.root())
      .stdin(Stdio::null());
    // Coverage runs keep their profile path, so no `.profraw` lands in the cwd.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

fn stderr(out: &Output) -> String {
  String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn scopes_are_checked_per_host() {
  let sb = Sb::new().unwrap();
  let codex = sb
    .run(&["plugins", "install", "--scope", "user", "--host", "codex"])
    .unwrap();
  assert_eq!(codex.status.code(), Some(64));
  assert!(stderr(&codex).contains("codex has no scope concept"));
  let bad = sb
    .run(&["plugins", "install", "--scope", "global"])
    .unwrap();
  assert_eq!(bad.status.code(), Some(64));
}

#[test]
fn config_replays_in_dependency_order_and_refuses_a_newer_major() {
  let sb = Sb::new().unwrap();
  let selection = sb
    .write(
      "plugins.json",
      r#"{"version":1,"enabled":["rust-quality"]}"#,
    )
    .unwrap();
  let path = selection.display().to_string();
  let out = sb
    .run(&[
      "plugins",
      "install",
      "--config",
      &path,
      "--dry-run",
      "--host",
      "claude",
    ])
    .unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
  assert_eq!(
    String::from_utf8_lossy(&out.stdout),
    "Planned commands (nothing was run):\n  claude plugin install toolu@toolu\n  claude plugin install rust-quality@toolu\n"
  );
  let newer = sb
    .write(
      "newer.json",
      r#"{"version":1,"generatorVersion":"99.0.0","enabled":[]}"#,
    )
    .unwrap();
  let refused = sb
    .run(&[
      "plugins",
      "install",
      "--config",
      &newer.display().to_string(),
      "--host",
      "claude",
    ])
    .unwrap();
  assert_eq!(refused.status.code(), Some(64));
  assert!(
    stderr(&refused).contains("was written by toolu 99.0.0"),
    "{}",
    stderr(&refused)
  );
  let both = sb
    .run(&["plugins", "install", "jev", "--config", &path])
    .unwrap();
  assert_eq!(both.status.code(), Some(64));
}

#[test]
fn no_input_with_two_hosts_names_both_and_never_prompts() {
  let sb = Sb::new().unwrap();
  sb.stub("claude", "[]").unwrap();
  sb.stub("codex", r#"{"installed":[]}"#).unwrap();
  for args in [
    &["plugins", "install", "--no-input"][..],
    &["plugins", "list"][..],
  ] {
    let out = sb.run(args).unwrap();
    assert_eq!(out.status.code(), Some(64), "{args:?}");
    assert!(
      stderr(&out).contains("several hosts found (claude, codex). Choose one with --host."),
      "{}",
      stderr(&out)
    );
  }
}

#[test]
fn unsupported_hosts_and_unconfirmed_removals_are_usage_errors() {
  let sb = Sb::new().unwrap();
  let hermes = sb.run(&["--host", "hermes", "plugins", "list"]).unwrap();
  assert_eq!(hermes.status.code(), Some(64));
  assert!(
    stderr(&hermes).contains("not an installer target"),
    "{}",
    stderr(&hermes)
  );
  let json = sb
    .run(&["--json", "--host", "hermes", "plugins", "list"])
    .unwrap();
  let doc: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(doc["error"]["code"], 64);
  let unconfirmed = sb
    .run(&["plugins", "remove", "jev", "--host", "claude"])
    .unwrap();
  assert_eq!(unconfirmed.status.code(), Some(64));
  assert!(stderr(&unconfirmed).contains("remove requires --yes to confirm"));
  let unknown = sb
    .run(&[
      "plugins",
      "install",
      "nope",
      "--host",
      "claude",
      "--dry-run",
    ])
    .unwrap();
  assert_eq!(unknown.status.code(), Some(64));
  assert!(
    stderr(&unknown).contains("unknown plugin: nope. Run `toolu plugins list` for the catalog.")
  );
  let none = sb.run(&["plugins", "list"]).unwrap();
  assert_eq!(none.status.code(), Some(64));
  assert!(stderr(&none).contains("no host found on PATH (probed claude, codex, muse)"));
}

#[test]
fn a_catalog_file_replaces_the_built_in_one() {
  let sb = Sb::new().unwrap();
  let catalog = sb
    .write("m.json", r#"{"name":"t","plugins":[{"name":"only"}]}"#)
    .unwrap();
  let out = sb
    .run(&[
      "plugins",
      "install",
      "--catalog",
      &catalog.display().to_string(),
      "--dry-run",
      "--host",
      "codex",
    ])
    .unwrap();
  assert_eq!(
    String::from_utf8_lossy(&out.stdout),
    "Planned commands (nothing was run):\n  codex plugin add only@toolu\n"
  );
  let bad = sb.write("bad.json", "[]").unwrap();
  let malformed = sb
    .run(&[
      "plugins",
      "list",
      "--catalog",
      &bad.display().to_string(),
      "--host",
      "codex",
    ])
    .unwrap();
  assert_eq!(malformed.status.code(), Some(64));
}
