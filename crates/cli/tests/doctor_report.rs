//! `toolu doctor` reports plugins, skew, config, tools and the registry.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

const CLEAR: &str = "TOOLU_CONFIG_DIR TOOLU_USER_CONFIG_DIR CLAUDE_CONFIG_DIR CLAUDE_PROJECT_DIR \
CLAUDE_PLUGINS_REGISTRY CODEX_HOME CURSOR_PROJECT_DIR HERMES_HOME XDG_CONFIG_HOME \
TOOLU_OPENCODE_HOME TOOLU_CODEX_PLUGIN_SNAPSHOT TOOLU_BUN TOOLU_EPIC_TOKEN \
TOOLU_EPIC_STATUS_TOKEN TOOLU_EPIC_PEER_TOKENS TOOLU_EPIC_NOTIFY_URL";

const CHECKS: &str = "binary,reachability,runtime,host,config,plugins,skew,registry,tools";

fn repo() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let dest = to.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      copy_dir(&entry.path(), &dest)?;
    } else {
      std::fs::copy(entry.path(), &dest)?;
    }
  }
  Ok(())
}

fn copy_plugin(name: &str, dest: &Path) -> std::io::Result<PathBuf> {
  let to = dest.join(name);
  copy_dir(&repo().join("plugins").join(name), &to)?;
  Ok(to)
}

fn doctor(home: &Path, host: &str, path: &str) -> std::io::Result<Output> {
  let mut command = Command::new(TOOLU);
  command
    .args(["--host", host, "--json", "doctor"])
    .env("HOME", home)
    .env("PATH", path)
    .env("TOOLU_PROJECT_DIR", home);
  for key in CLEAR.split(' ') {
    command.env_remove(key);
  }
  command.output()
}

fn parsed(output: &Output) -> Option<Value> {
  serde_json::from_slice(&output.stdout).ok()
}

fn check<'a>(document: &'a Value, id: &str) -> Option<&'a Value> {
  document
    .get("checks")?
    .as_array()?
    .iter()
    .find(|check| check.get("id").and_then(Value::as_str) == Some(id))
}

fn assert_order(document: &Value) {
  let ids: Vec<&str> = document
    .get("checks")
    .and_then(Value::as_array)
    .map(|checks| {
      checks
        .iter()
        .filter_map(|check| check.get("id").and_then(Value::as_str))
        .collect()
    })
    .unwrap_or_default();
  assert_eq!(ids.join(","), CHECKS);
}

fn claude_record(home: &Path, plugins: &[(&str, &Path)]) -> std::io::Result<()> {
  let mut map = serde_json::Map::new();
  for (name, path) in plugins {
    map.insert(
      format!("{name}@toolu"),
      json!([{ "scope": "user", "installPath": path.display().to_string() }]),
    );
  }
  let file = home.join(".claude/plugins/installed_plugins.json");
  if let Some(parent) = file.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(file, json!({ "version": 2, "plugins": map }).to_string())?;
  Ok(())
}

fn shell_path(front: Option<&Path>) -> String {
  let dir = Path::new(TOOLU).parent().unwrap_or(Path::new("."));
  match front {
    Some(front) => format!("{}:{}:/usr/bin:/bin", front.display(), dir.display()),
    None => format!("{}:/usr/bin:/bin", dir.display()),
  }
}

#[test]
fn claude_lists_copied_plugins_in_check_order() {
  let home = tempfile::tempdir().unwrap();
  let toolu = copy_plugin("toolu", home.path()).unwrap();
  let babysit = copy_plugin("pr-babysit", home.path()).unwrap();
  claude_record(home.path(), &[("toolu", &toolu), ("pr-babysit", &babysit)]).unwrap();
  let output = doctor(home.path(), "claude", &shell_path(None)).unwrap();
  let document = parsed(&output).unwrap();
  assert_order(&document);
  let plugins = check(&document, "plugins").unwrap()["details"]["plugins"]
    .as_array()
    .unwrap();
  assert_eq!(plugins.len(), 2);
  for (name, root) in [("toolu", &toolu), ("pr-babysit", &babysit)] {
    let plugin = plugins
      .iter()
      .find(|plugin| plugin["name"] == name)
      .unwrap();
    assert_eq!(plugin["version"], "7.11.0");
    assert_eq!(plugin["root"], root.display().to_string());
  }
}

#[test]
fn codex_lists_the_stub_and_opencode_is_names_only() {
  let home = tempfile::tempdir().unwrap();
  let toolu = copy_plugin("toolu", home.path()).unwrap();
  let bin = home.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  let stub = bin.join("codex");
  std::fs::write(
    &stub,
    format!(
      "#!/bin/sh\nprintf '%s\\n' '{{\"installed\":[{{\"name\":\"toolu\",\"marketplaceName\":\"toolu\",\"source\":{{\"path\":\"{}\"}}}}]}}'\n",
      toolu.display()
    ),
  )
  .unwrap();
  let mut mode = std::fs::metadata(&stub).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&stub, mode).unwrap();
  let codex = parsed(&doctor(home.path(), "codex", &shell_path(Some(&bin))).unwrap()).unwrap();
  assert_order(&codex);
  let listed = &check(&codex, "plugins").unwrap()["details"]["plugins"][0];
  assert_eq!(listed["version"], "7.11.0");
  assert_eq!(listed["root"], toolu.display().to_string());

  let status = home
    .path()
    .join(".config/opencode/toolu/opencode-status.json");
  std::fs::create_dir_all(status.parent().unwrap()).unwrap();
  std::fs::write(&status, r#"{"plugins":[{"name":"toolu"}]}"#).unwrap();
  let opencode = parsed(&doctor(home.path(), "opencode", &shell_path(None)).unwrap()).unwrap();
  assert_order(&opencode);
  assert_eq!(
    check(&opencode, "plugins").unwrap()["details"]["plugins"][0],
    json!({"name": "toolu"})
  );
  assert_eq!(check(&opencode, "skew").unwrap()["status"], "ok");
  assert_eq!(
    check(&opencode, "skew").unwrap()["summary"],
    "not applicable"
  );
}

#[test]
fn cursor_warns_that_plugin_inventory_is_unknown() {
  let home = tempfile::tempdir().unwrap();
  let cursor = parsed(&doctor(home.path(), "cursor", &shell_path(None)).unwrap()).unwrap();
  let plugins = check(&cursor, "plugins").unwrap();
  assert_eq!(plugins["status"], "warn");
  assert!(plugins["summary"].as_str().unwrap().contains("unknown"));
}

#[test]
fn an_unknown_config_key_fails_the_config_check() {
  let home = tempfile::tempdir().unwrap();
  let path = home.path().join(".claude/toolu.config.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, r#"{"bogus":1}"#).unwrap();
  let output = doctor(home.path(), "claude", &shell_path(None)).unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  let document = parsed(&output).unwrap();
  let config = check(&document, "config").unwrap();
  assert_eq!(config["status"], "fail");
  let summary = config["summary"].as_str().unwrap();
  assert!(
    summary.contains("unknown top-level key 'bogus'"),
    "{summary}"
  );
  assert!(summary.contains(&path.display().to_string()), "{summary}");
  assert!(String::from_utf8_lossy(&output.stderr).contains("check"));
}

#[test]
fn version_skew_warns_and_protocol_skew_fails() {
  let home = tempfile::tempdir().unwrap();
  let toolu = copy_plugin("toolu", home.path()).unwrap();
  let manifest = toolu.join(".claude-plugin/plugin.json");
  let mut plugin: Value =
    serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
  plugin["version"] = json!("0.0.1");
  std::fs::write(&manifest, plugin.to_string()).unwrap();
  claude_record(home.path(), &[("toolu", &toolu)]).unwrap();
  let warned = doctor(home.path(), "claude", &shell_path(None)).unwrap();
  assert_eq!(warned.status.code(), Some(0), "{warned:?}");
  let warned = parsed(&warned).unwrap();
  assert_eq!(check(&warned, "skew").unwrap()["status"], "warn");
  assert!(
    check(&warned, "skew").unwrap()["summary"]
      .as_str()
      .unwrap()
      .contains("0.0.1")
  );

  plugin["version"] = json!("7.11.0");
  plugin["hookProtocol"] = json!(99);
  std::fs::write(&manifest, plugin.to_string()).unwrap();
  let failed = doctor(home.path(), "claude", &shell_path(None)).unwrap();
  assert_eq!(failed.status.code(), Some(1), "{failed:?}");
  let failed = parsed(&failed).unwrap();
  assert_eq!(check(&failed, "skew").unwrap()["status"], "fail");
  assert!(
    check(&failed, "skew").unwrap()["summary"]
      .as_str()
      .unwrap()
      .contains("99")
  );
}

#[test]
fn pr_babysit_without_gh_fails_tools() {
  let home = tempfile::tempdir().unwrap();
  let babysit = copy_plugin("pr-babysit", home.path()).unwrap();
  claude_record(home.path(), &[("pr-babysit", &babysit)]).unwrap();
  let bin = home.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  std::fs::write(bin.join("git"), "").unwrap();
  let path = bin.display().to_string();
  let output = doctor(home.path(), "claude", &path).unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  let document = parsed(&output).unwrap();
  let tools = check(&document, "tools").unwrap();
  assert_eq!(tools["status"], "fail");
  assert!(
    tools["summary"]
      .as_str()
      .unwrap()
      .contains("pr-babysit needs gh"),
    "{}",
    tools["summary"]
  );
  assert!(
    tools["hint"]
      .as_str()
      .unwrap()
      .contains("https://cli.github.com")
  );
}

#[test]
fn registry_warns_on_javascript_and_orphans_and_fails_a_bad_manifest() {
  let home = tempfile::tempdir().unwrap();
  let dir = home.path().join(".claude/toolu/pre-tools.d");
  std::fs::create_dir_all(&dir).unwrap();
  std::fs::write(dir.join("x@vendor__mod.js"), "").unwrap();
  std::fs::write(dir.join("x@vendor__mod.sh"), "").unwrap();
  std::fs::write(dir.join("loose.js"), "").unwrap();
  std::fs::write(
    dir.join("jev@toolu__rule.json"),
    r#"{"version":1,"spec":"jev@toolu","name":"rule","event":"tool/pre","matcher":"*"}"#,
  )
  .unwrap();
  std::fs::write(
    dir.join("x@vendor__bad.json"),
    r#"{"version":2,"spec":"x@vendor","name":"bad","event":"tool/pre","matcher":"*"}"#,
  )
  .unwrap();
  std::fs::create_dir_all(home.path().join(".claude/plugins")).unwrap();
  std::fs::write(
    home.path().join(".claude/plugins/installed_plugins.json"),
    r#"{"version":2,"plugins":{}}"#,
  )
  .unwrap();
  let output = doctor(home.path(), "claude", &shell_path(None)).unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  let document = parsed(&output).unwrap();
  let registry = check(&document, "registry").unwrap();
  let summary = registry["summary"].as_str().unwrap();
  assert_eq!(registry["status"], "fail");
  assert!(
    summary.contains("runs through the Bun bridge until #440"),
    "{summary}"
  );
  assert!(
    summary.contains("jev@toolu__rule.json is an orphaned manifest"),
    "{summary}"
  );
  assert!(summary.contains("unsupported version 2"), "{summary}");
  assert!(summary.contains("loose.js is unnamespaced"), "{summary}");
  assert!(!summary.contains("x@vendor__mod.sh"), "{summary}");
}
