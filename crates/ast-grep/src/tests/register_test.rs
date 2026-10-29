use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::register;

#[test]
fn register_writes_real_valid_manifests_twice() {
  let dir = tempfile::tempdir().expect("config root");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().display().to_string())]);
  for _ in 0..2 {
    let result = register(&env, Host::Claude, "{}");
    assert_eq!(result.exit.code(), 0);
  }
  let pre = dir
    .path()
    .join("toolu/pre-tools.d/ast-grep@toolu__search-nudge.json");
  let post = dir
    .path()
    .join("toolu/post-tools.d/ast-grep@toolu__byte-savings.json");
  assert!(pre.is_file() && post.is_file());
  let read = toolu_runtime::registry::manifest::read_manifest(
    &pre,
    toolu_runtime::registry::RegistryEvent::ToolPre,
  )
  .expect("valid manifest");
  assert_eq!(read.matcher, "Grep|Bash|Shell");
}

#[test]
fn register_replaces_stale_module_with_migration_hint_once() {
  let dir = tempfile::tempdir().expect("config root");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().display().to_string())]);
  let registry = dir.path().join("toolu/pre-tools.d");
  std::fs::create_dir_all(&registry).expect("registry");
  let old = registry.join("ast-grep@toolu__search-nudge.js");
  std::fs::write(&old, "old bundle").expect("old module");
  let first = register(&env, Host::Claude, "{}");
  assert_eq!(first.exit.code(), 0);
  assert!(!old.exists());
  assert!(
    first
      .stdout
      .is_some_and(|message| message.contains("toolu ast-grep search"))
  );
  assert!(register(&env, Host::Claude, "{}").stdout.is_none());
}

#[test]
fn register_reports_native_manifests_to_startup_verifier() {
  let dir = tempfile::tempdir().expect("config root");
  let report = dir.path().join("startup.jsonl");
  std::fs::write(&report, "").expect("report");
  let env = Env::from_pairs([
    ("TOOLU_CONFIG_DIR", dir.path().display().to_string()),
    ("TOOLU_STARTUP_REPORT", report.display().to_string()),
  ]);
  assert_eq!(register(&env, Host::Cursor, "{}").exit.code(), 0);
  let lines = std::fs::read_to_string(&report).expect("report lines");
  let records: Vec<serde_json::Value> = lines
    .lines()
    .map(|line| serde_json::from_str(line).expect("record JSON"))
    .collect();
  assert_eq!(records.len(), 2);
  assert_eq!(records[0]["kind"], "native-registry");
  assert_eq!(records[0]["name"], "search-nudge");
  assert_eq!(records[0]["event"], "tool/pre");
  assert_eq!(records[1]["name"], "byte-savings");
  assert_eq!(records[1]["event"], "tool/post");
  for record in records {
    let path = record["target"].as_str().expect("target");
    assert!(std::path::Path::new(path).is_file());
  }
}

#[test]
fn register_reports_a_failed_write_without_failing_the_session() {
  let dir = tempfile::tempdir().expect("scratch");
  let file = dir.path().join("plain-file");
  std::fs::write(&file, "").expect("plain file");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", file.display().to_string())]);
  let failed = register(&env, Host::Claude, "{}");
  assert_eq!(failed.exit.code(), 0);
  assert!(failed.stderr.is_none());
  let stdout = failed.stdout.expect("systemMessage");
  let json: serde_json::Value = serde_json::from_str(&stdout).expect("one JSON document");
  let message = json["systemMessage"].as_str().expect("message");
  assert!(
    message.starts_with(&format!(
      "ast-grep registration failed: {}",
      file.join("toolu/pre-tools.d").display()
    )),
    "{message}"
  );
}

#[test]
fn a_failure_after_an_old_module_was_removed_still_shows_the_migration_notice() {
  let dir = tempfile::tempdir().expect("config root");
  let missing = dir.path().join("no-such-dir/startup.jsonl");
  let env = Env::from_pairs([
    ("TOOLU_CONFIG_DIR", dir.path().display().to_string()),
    ("TOOLU_STARTUP_REPORT", missing.display().to_string()),
  ]);
  let registry = dir.path().join("toolu/pre-tools.d");
  std::fs::create_dir_all(&registry).expect("registry");
  let old = registry.join("ast-grep@toolu__search-nudge.js");
  std::fs::write(&old, "old bundle").expect("old module");
  let outcome = register(&env, Host::Claude, "{}");
  assert_eq!(outcome.exit.code(), 0);
  assert!(!old.exists());
  let stdout = outcome.stdout.expect("systemMessage");
  let json: serde_json::Value = serde_json::from_str(&stdout).expect("one JSON document");
  let message = json["systemMessage"].as_str().expect("message");
  let lines: Vec<&str> = message.lines().collect();
  assert_eq!(lines.len(), 2, "{message}");
  assert!(lines[0].contains("toolu ast-grep search"), "{message}");
  assert!(
    lines[1].starts_with(&format!(
      "ast-grep registration failed: cannot open startup report {}: ",
      missing.display()
    )),
    "{message}"
  );
}
