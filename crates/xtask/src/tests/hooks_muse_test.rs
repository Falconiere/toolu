use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::launcher::Target;
use toolu_protocol::launcher::muse::{DEFAULT_TIMEOUT_MS, hook};

use super::check;
use crate::check_hooks::Finding;

const MANIFEST: &str = "plugins/toolu/.muse-plugin/plugin.json";

const HOOKS: [(&str, &str); 2] = [
  ("PreToolUse", "pre-tools"),
  ("SessionStart", "session-start"),
];

/// A plugin `toolu` whose Muse manifest and resolvers are what the generator writes.
fn generated() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let plugin = dir.path().join("plugins/toolu");
  let mut entries = Vec::new();
  for (event, name) in HOOKS {
    let target = Target {
      plugin: "toolu",
      event,
      name,
    };
    let generated = hook(&target, DEFAULT_TIMEOUT_MS).unwrap();
    write(&plugin.join(&generated.resolver_path), &generated.resolver);
    entries.push(serde_json::to_value(&generated.entry).unwrap());
  }
  let manifest = json!({
    "schemaVersion": 1,
    "name": "toolu",
    "version": "8.0.4",
    "description": "toolu",
    "compat": {"source": "native", "manifestDir": ".muse-plugin"},
    "capabilities": {"hooks": entries, "skills": [], "commands": []},
  });
  write_json(dir.path(), &manifest);
  dir
}

fn write(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

fn write_json(root: &Path, manifest: &Value) {
  let text = serde_json::to_string_pretty(manifest).unwrap();
  write(&root.join(MANIFEST), &text);
}

fn manifest(root: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(root.join(MANIFEST)).unwrap()).unwrap()
}

/// Apply `edit` to the first hook entry and judge the plugin.
fn edited(edit: impl FnOnce(&mut serde_json::Map<String, Value>)) -> Vec<Finding> {
  let dir = generated();
  let mut json = manifest(dir.path());
  edit(json["capabilities"]["hooks"][0].as_object_mut().unwrap());
  write_json(dir.path(), &json);
  check(dir.path(), "toolu")
}

fn only(found: &[Finding]) -> &Finding {
  assert_eq!(found.len(), 1, "{found:#?}");
  &found[0]
}

#[test]
fn generated_entries_and_resolvers_pass() {
  assert_eq!(check(generated().path(), "toolu"), Vec::new());
}

#[test]
fn a_plugin_without_muse_wiring_or_hooks_passes() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("plugins/toolu")).unwrap();
  assert_eq!(check(dir.path(), "toolu"), Vec::new());
  write_json(
    dir.path(),
    &json!({"name": "toolu", "capabilities": {"skills": []}}),
  );
  assert_eq!(check(dir.path(), "toolu"), Vec::new());
}

#[test]
fn a_swapped_argv_names_the_manifest_the_fix_and_the_expected_entry() {
  let found = edited(|entry| {
    entry["command"] = json!(["hooks/muse/pre-tools", "/bin/sh"]);
  });
  let finding = only(&found);
  assert_eq!(finding.file, MANIFEST);
  assert_eq!(finding.at, "capabilities.hooks[0]");
  assert_eq!(
    finding.problem,
    "entry differs from the generator; run cargo xtask print-muse-hook toolu PreToolUse pre-tools"
  );
  assert_eq!(
    finding.expected.as_deref(),
    Some(
      r#"{"id":"pre-tools","event":"PreToolUse","command":["/bin/sh","hooks/muse/pre-tools"],"timeoutMs":60000}"#
    )
  );
}

#[test]
fn an_added_matcher_or_status_message_differs_from_the_generator() {
  for (key, value) in [
    ("matcher", json!("Bash")),
    ("statusMessage", json!("toolu")),
  ] {
    let found = edited(|entry| {
      entry.insert(key.to_owned(), value);
    });
    assert!(only(&found).problem.starts_with("entry differs"), "{key}");
  }
}

#[test]
fn a_missing_or_bad_timeout_is_named_with_the_expected_entry() {
  for value in [
    None,
    Some(json!(0)),
    Some(json!(600_001)),
    Some(json!("60000")),
  ] {
    let found = edited(|entry| match value {
      Some(value) => {
        entry.insert("timeoutMs".to_owned(), value);
      }
      None => {
        entry.remove("timeoutMs");
      }
    });
    let finding = only(&found);
    assert!(
      finding
        .problem
        .starts_with("timeoutMs must be an integer from 1 to 600000; run cargo xtask"),
      "{finding}"
    );
    assert!(
      finding
        .expected
        .as_deref()
        .unwrap()
        .contains("\"timeoutMs\":60000")
    );
  }
}

#[test]
fn a_kept_custom_timeout_names_it_in_the_fix() {
  let dir = generated();
  let mut json = manifest(dir.path());
  json["capabilities"]["hooks"][0]["timeoutMs"] = json!(30_000);
  json["capabilities"]["hooks"][0]["command"] = json!(["sh", "hooks/muse/pre-tools"]);
  json["capabilities"]["hooks"][1]["event"] = json!("UserPromptSubmit");
  write_json(dir.path(), &json);
  let found = check(dir.path(), "toolu");
  let fixes: Vec<&str> = found.iter().map(|f| f.problem.as_str()).collect();
  assert_eq!(
    fixes,
    [
      "entry differs from the generator; run cargo xtask print-muse-hook toolu PreToolUse \
       pre-tools --timeout-ms 30000",
      "resolver differs from the generator; run cargo xtask print-muse-hook toolu \
       UserPromptSubmit session-start --resolver > plugins/toolu/hooks/muse/session-start",
    ],
    "{found:#?}"
  );
}

#[test]
fn an_unknown_event_or_bad_id_is_refused() {
  let found = edited(|entry| {
    entry["event"] = json!("Notification");
  });
  assert!(
    only(&found)
      .problem
      .starts_with("event must be a Muse event toolu handles")
  );
  let found = edited(|entry| {
    entry["id"] = json!("Pre_Tools");
  });
  let problems: Vec<&str> = found.iter().map(|f| f.problem.as_str()).collect();
  assert!(
    problems[0].starts_with("entry needs a string id"),
    "{found:#?}"
  );
  assert!(
    problems.iter().any(|p| p.starts_with("not referenced")),
    "{found:#?}"
  );
}

#[test]
fn a_duplicate_id_and_a_non_object_entry_are_named() {
  let dir = generated();
  let mut json = manifest(dir.path());
  let first = json["capabilities"]["hooks"][0].clone();
  json["capabilities"]["hooks"]
    .as_array_mut()
    .unwrap()
    .extend([first, json!("pre-tools")]);
  write_json(dir.path(), &json);
  let found = check(dir.path(), "toolu");
  let problems: Vec<(&str, &str)> = found
    .iter()
    .map(|f| (f.at.as_str(), f.problem.as_str()))
    .collect();
  assert_eq!(
    problems,
    [
      ("capabilities.hooks[2]", "duplicate hook id pre-tools"),
      ("capabilities.hooks[3]", "entry is not an object"),
    ]
  );
}

#[test]
fn a_hook_protocol_field_or_bad_shape_is_refused() {
  let dir = generated();
  let mut json = manifest(dir.path());
  json["hookProtocol"] = json!(2);
  write_json(dir.path(), &json);
  let finding = only(&check(dir.path(), "toolu")).clone();
  assert_eq!(finding.at, "");
  assert!(
    finding
      .problem
      .starts_with("Muse manifests carry no hookProtocol")
  );
  json["capabilities"]["hooks"] = json!({});
  write_json(dir.path(), &json);
  let problems: Vec<String> = check(dir.path(), "toolu")
    .into_iter()
    .map(|f| f.problem)
    .collect();
  assert!(problems.contains(&"capabilities.hooks is not an array".to_owned()));
  write(&dir.path().join(MANIFEST), "{");
  assert!(
    only(&check(dir.path(), "toolu"))
      .problem
      .starts_with("invalid JSON")
  );
}

#[test]
fn a_stale_or_missing_resolver_names_the_resolver_and_its_fix() {
  let dir = generated();
  let resolver = dir.path().join("plugins/toolu/hooks/muse/pre-tools");
  std::fs::write(&resolver, "exec toolu hook pre-tools\n").unwrap();
  let finding = only(&check(dir.path(), "toolu")).clone();
  assert_eq!(finding.file, "plugins/toolu/hooks/muse/pre-tools");
  assert_eq!(
    finding.problem,
    "resolver differs from the generator; run cargo xtask print-muse-hook toolu PreToolUse \
     pre-tools --resolver > plugins/toolu/hooks/muse/pre-tools"
  );
  std::fs::remove_file(&resolver).unwrap();
  assert!(
    only(&check(dir.path(), "toolu"))
      .problem
      .starts_with("resolver is missing; run cargo xtask")
  );
}

#[test]
fn an_orphan_resolver_fails_with_or_without_a_manifest() {
  let dir = generated();
  write(&dir.path().join("plugins/toolu/hooks/muse/stray"), "true\n");
  let finding = only(&check(dir.path(), "toolu")).clone();
  assert_eq!(finding.file, "plugins/toolu/hooks/muse/stray");
  assert_eq!(
    finding.problem,
    "not referenced by .muse-plugin/plugin.json; remove it or add its entry"
  );
  std::fs::remove_file(dir.path().join(MANIFEST)).unwrap();
  assert_eq!(check(dir.path(), "toolu").len(), 3);
}
