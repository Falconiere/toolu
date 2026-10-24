//! `toolu doctor` with a plugin the marketplace retired (#615): the skew check
//! warns with the command that removes it, that command removes it through the
//! host's CLI, and doctor then passes. A stub `claude` stands in at the process
//! boundary, doing what the real `claude plugin uninstall` does to
//! `installed_plugins.json`.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const CLEAR: &str = "TOOLU_CONFIG_DIR TOOLU_USER_CONFIG_DIR CLAUDE_CONFIG_DIR CLAUDE_PROJECT_DIR \
CLAUDE_PLUGINS_REGISTRY CODEX_HOME CURSOR_PROJECT_DIR HERMES_HOME XDG_CONFIG_HOME \
TOOLU_CODEX_PLUGIN_SNAPSHOT TOOLU_BIN";

/// context7's `plugin.json` as 7.11.0 installed it, before plugins declared a
/// `hookProtocol` (captured from a real `~/.claude/plugins/cache`).
const CONTEXT7_7_11_0: &str = r#"{
  "name": "context7",
  "version": "7.11.0",
  "description": "Library documentation & code-example lookup via Context7 (skill + REST wrapper). Standalone, no dependencies.",
  "author": {
    "name": "Falconiere Barbosa",
    "email": "hello@falconiere.io"
  },
  "homepage": "https://github.com/falconiere/toolu",
  "repository": "https://github.com/falconiere/toolu.git",
  "license": "MIT",
  "keywords": [
    "claude-code",
    "context7",
    "documentation",
    "skills"
  ]
}"#;

/// A Claude Code home holding the repository's toolu manifest and context7 at
/// 7.11.0, each under `cache/<name>`.
struct Home {
  dir: tempfile::TempDir,
}

impl Home {
  fn new() -> Res<Home> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-doctor-retired-")
      .tempdir()?;
    let home = Home { dir };
    std::fs::create_dir_all(home.path().join("bin"))?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu");
    let toolu = std::fs::read_to_string(repo.join(".claude-plugin/plugin.json"))?;
    home.manifest("toolu", &toolu)?;
    home.manifest("context7", CONTEXT7_7_11_0)?;
    Ok(home)
  }

  fn path(&self) -> &Path {
    self.dir.path()
  }

  fn root(&self, name: &str) -> PathBuf {
    self.path().join("cache").join(name)
  }

  fn manifest(&self, name: &str, text: &str) -> Res<()> {
    let dir = self.root(name).join(".claude-plugin");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("plugin.json"), text)?;
    Ok(())
  }

  fn record_path(&self) -> PathBuf {
    self.path().join(".claude/plugins/installed_plugins.json")
  }

  /// `installed_plugins.json` naming `names` at user scope.
  fn record(&self, names: &[&str]) -> String {
    let plugins: serde_json::Map<String, Value> = names
      .iter()
      .map(|name| {
        let path = self.root(name).display().to_string();
        (
          format!("{name}@toolu"),
          json!([{ "scope": "user", "installPath": path }]),
        )
      })
      .collect();
    json!({ "version": 2, "plugins": plugins }).to_string()
  }

  fn write_record(&self, text: &str) -> Res<()> {
    let path = self.record_path();
    if let Some(dir) = path.parent() {
      std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    Ok(())
  }

  /// A `claude` that logs its argv and, for `plugin uninstall context7@toolu`,
  /// writes `after` as the install record; any other command fails.
  fn stub_claude(&self, after: &str) -> Res<()> {
    let root = self.path().display().to_string();
    let record = self.record_path().display().to_string();
    let body = format!(
      "#!/bin/sh\necho \"$*\" >> {root}/claude.log\ncase \"$*\" in\n\"plugin uninstall context7@toolu\") printf '%s' '{after}' > {record} ;;\n*) echo \"unexpected: $*\" >&2; exit 1 ;;\nesac\n"
    );
    let temp = self.path().join("bin/.claude");
    std::fs::write(&temp, body)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&temp, self.path().join("bin/claude"))?;
    Ok(())
  }

  fn log(&self) -> Vec<String> {
    std::fs::read_to_string(self.path().join("claude.log"))
      .unwrap_or_default()
      .lines()
      .map(str::to_owned)
      .collect()
  }

  fn toolu(&self, args: &[&str]) -> Res<Output> {
    let dir = Path::new(TOOLU).parent().unwrap_or(Path::new("."));
    let path = format!(
      "{}:{}:/usr/bin:/bin",
      self.path().join("bin").display(),
      dir.display()
    );
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .env("HOME", self.path())
      .env("PATH", path)
      .env("TOOLU_PROJECT_DIR", self.path());
    for key in CLEAR.split(' ') {
      command.env_remove(key);
    }
    Ok(command.output()?)
  }

  /// Doctor's exit code and its skew check.
  fn skew(&self) -> Res<(Option<i32>, Value)> {
    let output = self.toolu(&["--host", "claude", "--json", "doctor"])?;
    let document: Value = serde_json::from_slice(&output.stdout)?;
    let skew = document
      .get("checks")
      .and_then(Value::as_array)
      .and_then(|checks| {
        checks
          .iter()
          .find(|check| check.get("id").and_then(Value::as_str) == Some("skew"))
      })
      .cloned()
      .ok_or("doctor reported no skew check")?;
    Ok((output.status.code(), skew))
  }
}

#[test]
fn doctor_names_a_retired_plugin_and_the_command_it_prints_removes_it() {
  let home = Home::new().unwrap();
  home
    .write_record(&home.record(&["toolu", "context7"]))
    .unwrap();
  home.stub_claude(&home.record(&["toolu"])).unwrap();

  let (code, skew) = home.skew().unwrap();
  assert_eq!((code, &skew["status"]), (Some(0), &json!("warn")), "{skew}");
  assert_eq!(
    skew["summary"],
    "retired plugin context7 is no longer in the toolu marketplace"
  );
  assert_eq!(skew["details"]["retired"], json!(["context7"]));
  let hint = skew["hint"].as_str().unwrap();
  assert_eq!(hint, "toolu plugins remove context7 --host claude --yes");

  let words: Vec<&str> = hint.split(' ').collect();
  let removed = home.toolu(&words[1..]).unwrap();
  assert_eq!(
    removed.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&removed.stderr)
  );
  assert_eq!(home.log(), ["plugin uninstall context7@toolu"]);

  let (code, skew) = home.skew().unwrap();
  assert_eq!((code, &skew["status"]), (Some(0), &json!("ok")), "{skew}");
  assert_eq!(skew["summary"], "plugin versions match");
  assert_eq!(skew["details"]["retired"], json!([]));
}

#[test]
fn a_protocol_mismatch_still_fails_and_still_names_the_retired_plugin() {
  let home = Home::new().unwrap();
  let manifest = home.root("toolu").join(".claude-plugin/plugin.json");
  let mut toolu: Value =
    serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
  toolu["hookProtocol"] = json!(99);
  home.manifest("toolu", &toolu.to_string()).unwrap();
  home
    .write_record(&home.record(&["toolu", "context7"]))
    .unwrap();

  let (code, skew) = home.skew().unwrap();
  assert_eq!((code, &skew["status"]), (Some(1), &json!("fail")), "{skew}");
  let summary = skew["summary"].as_str().unwrap();
  assert!(summary.contains("hook protocol 99"), "{summary}");
  assert!(
    summary.ends_with("; retired plugin context7 is no longer in the toolu marketplace"),
    "{summary}"
  );
  assert_eq!(
    skew["hint"],
    "toolu plugins remove context7 --host claude --yes"
  );
}
