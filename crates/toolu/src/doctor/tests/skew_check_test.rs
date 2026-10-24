use std::path::Path;

use serde_json::json;

use super::super::checks::Status;
use super::super::inventory::collect;
use super::{check, removal};
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn install(home: &Path, version: &str, protocol: u32) -> Roots {
  let root = home.join("toolu");
  let manifest = root.join(".claude-plugin/plugin.json");
  std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
  std::fs::write(
    &manifest,
    format!(r#"{{"name":"toolu","version":"{version}","hookProtocol":{protocol}}}"#),
  )
  .unwrap();
  let record = home.join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(
    &record,
    format!(
      r#"{{"plugins":{{"toolu@toolu":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
      root.display()
    ),
  )
  .unwrap();
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.to_str().unwrap()),
    ]),
    Some(Host::Claude),
  )
}

#[test]
fn a_different_version_warns_and_a_different_protocol_fails() {
  let home = tempfile::tempdir().unwrap();
  let roots = install(home.path(), "0.0.1", HOOK_PROTOCOL);
  let warned = check(&collect(&roots, home.path()), &roots);
  assert_eq!(warned.status, Status::Warn, "{}", warned.summary);
  assert!(warned.summary.contains("0.0.1"), "{}", warned.summary);

  let roots = install(home.path(), "7.11.0", 99);
  let failed = check(&collect(&roots, home.path()), &roots);
  assert_eq!(failed.status, Status::Fail, "{}", failed.summary);
  assert!(
    failed.summary.contains("hook protocol 99"),
    "{}",
    failed.summary
  );
}

#[test]
fn no_plugins_make_skew_ok() {
  let home = tempfile::tempdir().unwrap();
  let record = home.path().join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(&record, r#"{"plugins":{}}"#).unwrap();
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", home.path().to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.path().to_str().unwrap()),
    ]),
    Some(Host::Claude),
  );
  let check = check(&collect(&roots, home.path()), &roots);
  assert_eq!(check.status, Status::Ok);
  assert_eq!(check.summary, "no plugins to compare");
}

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

fn current(name: &str) -> String {
  format!(
    r#"{{"name":"{name}","version":"{}","hookProtocol":{HOOK_PROTOCOL}}}"#,
    env!("CARGO_PKG_VERSION")
  )
}

/// Claude Code with each `(name, plugin.json)` installed for the user; a name
/// listed twice gets a second, project-scoped row for the same root.
fn claude(home: &Path, plugins: &[(&str, &str)]) -> Roots {
  let mut records = serde_json::Map::new();
  for (name, manifest) in plugins {
    let root = home.join("cache").join(name);
    std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    std::fs::write(root.join(".claude-plugin/plugin.json"), manifest).unwrap();
    let rows = records
      .entry(format!("{name}@toolu"))
      .or_insert_with(|| json!([]))
      .as_array_mut()
      .unwrap();
    let path = root.display().to_string();
    rows.push(if rows.is_empty() {
      json!({ "scope": "user", "installPath": path })
    } else {
      json!({ "scope": "project", "projectPath": home.display().to_string(), "installPath": path })
    });
  }
  let record = home.join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(
    &record,
    json!({ "version": 2, "plugins": records }).to_string(),
  )
  .unwrap();
  roots_for(home, Host::Claude)
}

fn roots_for(home: &Path, host: Host) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.to_str().unwrap()),
    ]),
    Some(host),
  )
}

#[test]
fn a_retired_plugin_warns_once_with_the_command_that_removes_it() {
  let home = tempfile::tempdir().unwrap();
  let toolu = current("toolu");
  let roots = claude(
    home.path(),
    &[
      ("toolu", &toolu),
      ("context7", CONTEXT7_7_11_0),
      ("context7", CONTEXT7_7_11_0),
    ],
  );
  let warned = check(&collect(&roots, home.path()), &roots);
  assert_eq!(warned.status, Status::Warn, "{}", warned.summary);
  assert_eq!(
    warned.summary,
    "retired plugin context7 is no longer in the toolu marketplace"
  );
  assert_eq!(
    warned.hint.as_deref(),
    Some("toolu plugins remove context7 --host claude --yes")
  );
  assert_eq!(warned.details, json!({ "retired": ["context7"] }));
}

#[test]
fn a_plugin_newer_than_this_binary_is_not_retired_but_an_older_one_is() {
  let home = tempfile::tempdir().unwrap();
  let newer =
    format!(r#"{{"name":"future-plugin","version":"999.0.0","hookProtocol":{HOOK_PROTOCOL}}}"#);
  let roots = claude(home.path(), &[("future-plugin", &newer)]);
  let advised = check(&collect(&roots, home.path()), &roots);
  assert_eq!(advised.status, Status::Warn, "{}", advised.summary);
  assert!(
    advised
      .summary
      .contains("is older than the future-plugin plugin 999.0.0"),
    "{}",
    advised.summary
  );
  assert_eq!(
    (advised.hint, advised.details),
    (None, json!({ "retired": [] }))
  );

  let older = newer.replace("999.0.0", "0.0.1");
  let roots = claude(home.path(), &[("future-plugin", &older)]);
  let retired = check(&collect(&roots, home.path()), &roots);
  assert_eq!(retired.details, json!({ "retired": ["future-plugin"] }));
}

#[test]
fn a_mismatch_still_fails_and_still_names_the_retired_plugins() {
  let home = tempfile::tempdir().unwrap();
  let toolu = current("toolu").replace(&format!(":{HOOK_PROTOCOL}}}"), ":99}");
  let roots = claude(
    home.path(),
    &[
      ("toolu", &toolu),
      ("context7", CONTEXT7_7_11_0),
      ("jira", &CONTEXT7_7_11_0.replace("context7", "jira")),
    ],
  );
  let failed = check(&collect(&roots, home.path()), &roots);
  assert_eq!(failed.status, Status::Fail, "{}", failed.summary);
  assert!(
    failed.summary.contains("hook protocol 99"),
    "{}",
    failed.summary
  );
  assert!(
    failed
      .summary
      .ends_with("; retired plugins context7, jira are no longer in the toolu marketplace"),
    "{}",
    failed.summary
  );
  assert_eq!(
    failed.hint.as_deref(),
    Some("toolu plugins remove context7 jira --host claude --yes")
  );
}

#[test]
fn the_removal_command_is_the_inspected_hosts() {
  let home = tempfile::tempdir().unwrap();
  let names = ["context7", "jira"];
  let hint = |host| removal(&roots_for(home.path(), host), &names);
  assert_eq!(
    hint(Host::Muse).as_deref(),
    Some("muse plugins remove context7; muse plugins remove jira")
  );
  assert_eq!(
    hint(Host::Codex).as_deref(),
    Some("toolu plugins remove context7 jira --host codex --yes")
  );
  assert_eq!(
    hint(Host::Cursor).as_deref(),
    Some("toolu plugins remove context7 jira --host cursor --yes")
  );
  assert_eq!(hint(Host::Hermes), None);
  assert_eq!(removal(&roots_for(home.path(), Host::Claude), &[]), None);
}
