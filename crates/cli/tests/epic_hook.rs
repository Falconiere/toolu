//! The three native epic-orchestrator `SessionStart` hooks from the real
//! binary: the Codex dependency notice, the native-binary advice and the
//! in-process engine start. All three exit 0.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A fallible helper result.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

const REQUIRED: [&str; 5] = [
  "toolu@toolu",
  "delivery-flow@toolu",
  "toolu-review@toolu",
  "pr-babysit@toolu",
  "brainstorm@toolu",
];

fn plugin_root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/epic-orchestrator")
}

/// A temp root whose `bin` holds the scripts a case puts on `PATH`.
struct Root {
  dir: tempfile::TempDir,
}

impl Root {
  fn new() -> Res<Self> {
    let root = Self {
      dir: tempfile::tempdir()?,
    };
    std::fs::create_dir_all(root.path("bin"))?;
    Ok(root)
  }

  fn path(&self, rel: &str) -> PathBuf {
    self.dir.path().join(rel)
  }

  fn script(&self, name: &str, body: &str) -> Res<()> {
    let path = self.path("bin").join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
  }

  fn codex(&self, listing: &Value) -> Res<()> {
    self.script("codex", &format!("cat <<'JSON'\n{listing}\nJSON"))
  }

  /// Run hook `name` with `stdin`, on Codex when `codex` is set.
  fn hook(&self, name: &str, stdin: &str, codex: bool) -> Res<Output> {
    let root = plugin_root();
    let mut command = Command::new(TOOLU);
    command
      .env_clear()
      .env(
        "PATH",
        format!("{}:/usr/bin:/bin", self.path("bin").display()),
      )
      .env("HOME", self.path("home"))
      .env("TOOLU_CONFIG_DIR", self.path("config"))
      .env("TOOLU_RESOURCE_HOME", self.path("resources"))
      .env("CLAUDE_PLUGIN_ROOT", &root)
      .args(["epic-orchestrator", "hook", name, "--event", "SessionStart"])
      .arg("--plugin-root")
      .arg(&root)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    if codex {
      command.env("PLUGIN_ROOT", &root);
    }
    // Coverage runs write each child's profile where the harness says.
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = command.spawn()?;
    child
      .stdin
      .take()
      .ok_or("no stdin")?
      .write_all(stdin.as_bytes())?;
    Ok(child.wait_with_output()?)
  }
}

fn out(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn installed(skip: &[usize], disabled: &[usize]) -> Value {
  let rows: Vec<Value> = REQUIRED
    .iter()
    .enumerate()
    .filter(|(index, _)| !skip.contains(index))
    .map(|(index, id)| json!({"pluginId": id, "installed": true, "enabled": !disabled.contains(&index)}))
    .collect();
  json!({"installed": rows})
}

#[test]
fn check_deps_names_every_missing_plugin_with_its_install_command() {
  let root = Root::new().unwrap();
  root.codex(&json!({"installed": []})).unwrap();
  let output = root.hook("check-deps", "{}", true).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let context = REQUIRED
    .iter()
    .map(|id| format!(" {id} (install with: codex plugin add {id})"))
    .collect::<Vec<_>>()
    .concat();
  // The key order is the host's: the event name first.
  let expected = format!(
    "{{\n  \"hookSpecificOutput\": {{\n    \"hookEventName\": \"SessionStart\",\n    \"additionalContext\": \"WARN: this plugin requires{context}\"\n  }}\n}}\n"
  );
  assert_eq!(out(&output), expected);
}

#[test]
fn check_deps_names_only_what_is_missing_or_disabled() {
  let root = Root::new().unwrap();
  root.codex(&installed(&[4], &[1])).unwrap();
  let output = root.hook("check-deps", "{}", true).unwrap();
  assert!(out(&output).contains(
    "WARN: this plugin requires delivery-flow@toolu (install with: codex plugin add delivery-flow@toolu) brainstorm@toolu (install with: codex plugin add brainstorm@toolu)\""
  ), "{}", out(&output));
}

#[test]
fn check_deps_is_silent_when_every_dependency_is_installed_and_enabled() {
  let root = Root::new().unwrap();
  root.codex(&installed(&[], &[])).unwrap();
  let output = root.hook("check-deps", "{}", true).unwrap();
  assert_eq!(
    (output.status.code(), out(&output)),
    (Some(0), String::new())
  );
  assert_eq!(output.stderr, b"");
}

#[test]
fn check_deps_is_silent_on_claude_code_whatever_codex_would_list() {
  let root = Root::new().unwrap();
  root.codex(&json!({"installed": []})).unwrap();
  let output = root.hook("check-deps", "{}", false).unwrap();
  assert_eq!(
    (output.status.code(), out(&output)),
    (Some(0), String::new())
  );
}

#[test]
fn check_binary_advises_once_per_session_id() {
  let root = Root::new().unwrap();
  let payload = r#"{"session_id":"epic-hook"}"#;
  let first = root.hook("check-binary", payload, false).unwrap();
  assert_eq!(first.status.code(), Some(0));
  let doc: Value = serde_json::from_str(&out(&first)).expect("json");
  let context = doc["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .expect("context");
  assert!(context.starts_with("toolu: native binary "), "{context}");
  assert_eq!(out(&root.hook("check-binary", payload, false).unwrap()), "");
  assert_ne!(
    out(
      &root
        .hook("check-binary", r#"{"session_id":"other"}"#, false)
        .unwrap()
    ),
    ""
  );
}

#[test]
fn check_binary_is_silent_when_native_toolu_is_on_path() {
  let root = Root::new().unwrap();
  root
    .script("toolu", "if [ \"$1\" = --hook-protocol ]; then echo 1; fi")
    .unwrap();
  let output = root
    .hook("check-binary", r#"{"session_id":"native"}"#, false)
    .unwrap();
  assert_eq!(
    (output.status.code(), out(&output)),
    (Some(0), String::new())
  );
}

#[test]
fn engine_ensure_is_silent_without_a_registered_epic() {
  let root = Root::new().unwrap();
  assert_eq!(out(&root.hook("engine-ensure", "{}", false).unwrap()), "");
  std::fs::create_dir_all(root.path("resources")).expect("resources");
  std::fs::write(
    root.path("resources/registry.json"),
    r#"{"version":1,"epics":[]}"#,
  )
  .expect("registry");
  let output = root.hook("engine-ensure", "{}", false).unwrap();
  assert_eq!(
    (output.status.code(), out(&output)),
    (Some(0), String::new())
  );
}

#[test]
fn engine_ensure_reports_a_malformed_registry_as_a_system_message() {
  let root = Root::new().unwrap();
  std::fs::create_dir_all(root.path("resources")).expect("resources");
  std::fs::write(root.path("resources/registry.json"), "{not json").expect("registry");
  let output = root.hook("engine-ensure", "{}", false).unwrap();
  assert_eq!(output.status.code(), Some(0));
  let doc: Value = serde_json::from_str(&out(&output)).expect("json");
  let message = doc["systemMessage"].as_str().expect("message");
  assert!(message.starts_with("epic engine: "), "{message}");
  assert!(message.contains("registry.json"), "{message}");
}
