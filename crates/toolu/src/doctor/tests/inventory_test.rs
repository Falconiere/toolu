use std::path::{Path, PathBuf};
use std::time::Duration;

use super::super::checks::Status;
use super::{CODEX_LIST_TIMEOUT, collect, parse_codex, plugins_check};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn claude(home: &Path, project: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", project.to_str().unwrap()),
    ]),
    Some(Host::Claude),
  )
}

fn write_plugin(root: &Path) {
  let path = root.join(".claude-plugin/plugin.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(
    path,
    r#"{"name":"toolu","version":"7.11.0","hookProtocol":1}"#,
  )
  .unwrap();
}

#[test]
fn claude_keeps_user_toolu_plugins_and_matching_project_scope() {
  let home = tempfile::tempdir().unwrap();
  let project = tempfile::tempdir().unwrap();
  let toolu = home.path().join("toolu");
  let babysit = home.path().join("pr-babysit");
  write_plugin(&toolu);
  write_plugin(&babysit);
  let record = home.path().join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  let project_path = project.path().display().to_string();
  std::fs::write(
    &record,
    format!(
      r#"{{"plugins":{{
        "toolu@toolu":[{{"scope":"user","installPath":"{}"}}],
        "other@elsewhere":[{{"scope":"user","installPath":"{}"}}],
        "pr-babysit@toolu":[{{"scope":"project","projectPath":"{project_path}","installPath":"{}"}}],
        "skipped@toolu":[{{"scope":"project","projectPath":"/elsewhere","installPath":"{}"}}]
      }}}}"#,
      toolu.display(),
      toolu.display(),
      babysit.display(),
      babysit.display(),
    ),
  )
  .unwrap();
  let roots = claude(home.path(), project.path());
  let inventory = collect(&roots, project.path());
  assert!(inventory.warning.is_none(), "{:?}", inventory.warning);
  let check = plugins_check(&inventory);
  assert_eq!(check.status, Status::Ok);
  let plugins = check.details["plugins"].as_array().unwrap();
  assert_eq!(plugins.len(), 2);
  let toolu = plugins
    .iter()
    .find(|plugin| plugin["name"] == "toolu")
    .unwrap();
  assert_eq!(toolu["version"], "7.11.0");
  assert!(toolu["root"].as_str().unwrap().contains("toolu"));
  assert!(plugins.iter().any(|plugin| plugin["name"] == "pr-babysit"));
}

#[test]
fn a_claude_entry_without_install_path_warns_and_is_omitted() {
  let home = tempfile::tempdir().unwrap();
  let record = home.path().join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(&record, r#"{"plugins":{"toolu@toolu":[{"scope":"user"}]}}"#).unwrap();
  let roots = claude(home.path(), home.path());
  let inventory = collect(&roots, home.path());
  let warning = inventory.warning.clone().unwrap_or_default();
  assert!(warning.contains("no installPath"), "{warning}");
  let plugins = plugins_check(&inventory).details["plugins"].clone();
  assert_eq!(plugins.as_array().unwrap().len(), 0);
}

#[test]
fn cursor_and_an_unreadable_record_warn() {
  let home = tempfile::tempdir().unwrap();
  let cursor = Roots::new(
    Env::from_pairs([("HOME", home.path().to_str().unwrap())]),
    Some(Host::Cursor),
  );
  let check = plugins_check(&collect(&cursor, home.path()));
  assert_eq!(check.status, Status::Warn);
  assert!(check.summary.contains("unknown"), "{}", check.summary);

  let claude = claude(home.path(), home.path());
  let missing = plugins_check(&collect(&claude, home.path()));
  assert_eq!(missing.status, Status::Warn);
  assert!(
    missing.summary.contains("unavailable"),
    "{}",
    missing.summary
  );
}

#[test]
fn opencode_lists_names_only() {
  let home = tempfile::tempdir().unwrap();
  let path = home
    .path()
    .join(".config/opencode/toolu/opencode-status.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(
    &path,
    r#"{"plugins":[{"name":"toolu"},{"name":"pr-babysit"}]}"#,
  )
  .unwrap();
  let roots = Roots::new(
    Env::from_pairs([("HOME", home.path().to_str().unwrap())]),
    Some(Host::Opencode),
  );
  let check = plugins_check(&collect(&roots, home.path()));
  assert_eq!(check.status, Status::Ok);
  assert_eq!(
    check.details["plugins"][0],
    serde_json::json!({"name": "toolu"})
  );
  assert!(check.details["plugins"][0].get("version").is_none());
  assert!(check.details["plugins"][0].get("root").is_none());
}

#[test]
fn codex_parse_keeps_enabled_toolu_plugins() {
  let root = PathBuf::from("/plugins/toolu");
  let list = parse_codex(&format!(
    r#"{{"installed":[
      {{"name":"toolu","marketplaceName":"toolu","source":{{"path":"{}"}}}},
      {{"name":"off","marketplaceName":"toolu","enabled":false,"source":{{"path":"/x"}}}},
      {{"name":"other","marketplaceName":"elsewhere","source":{{"path":"/y"}}}}
    ]}}"#,
    root.display()
  ))
  .unwrap();
  assert_eq!(list.plugins.len(), 1);
  assert_eq!(list.plugins[0].name, "toolu");
  assert_eq!(list.missing_path, 0);
  assert_eq!(CODEX_LIST_TIMEOUT, Duration::from_secs(10));
}

#[test]
fn codex_without_a_cli_warns_without_waiting() {
  let home = tempfile::tempdir().unwrap();
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", home.path().to_str().unwrap()),
      ("PATH", home.path().to_str().unwrap()),
    ]),
    Some(Host::Codex),
  );
  let started = std::time::Instant::now();
  let check = plugins_check(&collect(&roots, home.path()));
  assert!(
    started.elapsed() < Duration::from_secs(5),
    "codex list waited {:?}",
    started.elapsed()
  );
  assert_eq!(check.status, Status::Warn);
  assert!(check.summary.contains("unavailable"), "{}", check.summary);
}
