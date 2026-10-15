//! A third-party `.js` registry module (#440): `toolu hook pre-tools` never
//! runs it, even with a `bun` on `PATH`, while its spec's `.sh` still runs, and
//! `toolu hook session-start` names it once with the executable protocol.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Output;

/// The `toolu` binary under test.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

const BASH: &str = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;
const STARTUP: &str = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;

/// Write `body` at `path`, executable when `exec`.
fn write(path: &Path, body: &str, exec: bool) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, body)?;
  if exec {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  }
  Ok(())
}

/// `home/.claude/toolu/pre-tools.d` with a denying `.js`, an allowing `.sh` of
/// the same spec, and a `bun` on `PATH` that records being run.
struct Sandbox {
  _dir: tempfile::TempDir,
  root: PathBuf,
}

impl Sandbox {
  fn new() -> Res<Sandbox> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?;
    let pre = root.join("home/.claude/toolu/pre-tools.d");
    write(
      &pre.join("x@t__deny.js"),
      "export default { spec: \"x@t\", name: \"deny\", event: \"tool/pre\", \
       async run() { return { kind: \"deny\", reason: \"js denies\" }; } };",
      false,
    )?;
    write(
      &pre.join("x@t__ok.sh"),
      "#!/usr/bin/env bash\nprintf '%s\\n' '{\"systemMessage\":\"sh ran\"}'\n",
      true,
    )?;
    let marker = root.join("bun-ran");
    write(
      &root.join("bin/bun"),
      &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
      true,
    )?;
    std::fs::create_dir_all(root.join("project"))?;
    Ok(Sandbox { _dir: dir, root })
  }

  fn hook(&self, name: &str, event: &str, stdin: &str) -> Res<Output> {
    let system = std::env::var("PATH")?;
    let bin = self.root.join("bin");
    Ok(
      assert_cmd::Command::new(TOOLU)
        .args(["hook", name, "--event", event])
        .env_clear()
        .env("PATH", format!("{}:{system}", bin.display()))
        .env("TOOLU_BUN", bin.join("bun"))
        .env("HOME", self.root.join("home"))
        .env("CLAUDE_PROJECT_DIR", self.root.join("project"))
        .current_dir(self.root.join("project"))
        .write_stdin(stdin)
        .output()?,
    )
  }

  fn bun_ran(&self) -> bool {
    self.root.join("bun-ran").exists()
  }
}

#[test]
fn a_js_deny_never_runs_and_the_sh_does() {
  let sb = Sandbox::new().unwrap();
  let out = sb.hook("pre-tools", "PreToolUse", BASH).unwrap();
  let stdout = String::from_utf8_lossy(&out.stdout);
  assert_eq!(out.status.code(), Some(0), "{out:?}");
  assert!(stdout.contains("sh ran"), "{stdout}");
  assert!(!stdout.contains("deny"), "{stdout}");
  assert_eq!(String::from_utf8_lossy(&out.stderr), "");
  assert!(!sb.bun_ran(), "bun was run");
}

#[test]
fn session_start_names_the_js_module_once() {
  let sb = Sandbox::new().unwrap();
  let out = sb.hook("session-start", "SessionStart", STARTUP).unwrap();
  assert_eq!(out.status.code(), Some(0), "{out:?}");
  let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let message = doc["systemMessage"].as_str().unwrap_or_default();
  let line = "toolu-registry: pre-tools.d/x@t__deny.js is a JavaScript module, which toolu no \
              longer runs; publish it as an executable x@t__deny.sh \
              (docs/registry.md#executable-modules)";
  assert_eq!(message.matches(line).count(), 1, "{message}");
  assert!(!message.contains("x@t__ok.sh"), "{message}");
  assert!(!sb.bun_ran(), "bun was run");
}
