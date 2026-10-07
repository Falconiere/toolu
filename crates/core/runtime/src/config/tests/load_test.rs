use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{ConfigFiles, LoadedConfig, WARN_PREFIX, check_text, exists, load, merge};
use crate::env::Env;
use crate::host::roots::Roots;

fn fixture(name: &str) -> String {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../../fixtures/config")
    .join(name);
  std::fs::read_to_string(path).unwrap()
}

/// A sandbox with `home` and `project`, and roots that find both.
fn sandbox(host: Host) -> (tempfile::TempDir, Roots) {
  let dir = tempfile::tempdir().unwrap();
  let (home, project) = (dir.path().join("home"), dir.path().join("project"));
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_PROJECT_DIR", project.display().to_string()),
  ]);
  (dir, Roots::new(env, Some(host)))
}

fn place(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

fn loaded(user: Option<&str>, project: Option<&str>) -> (tempfile::TempDir, LoadedConfig) {
  let (dir, roots) = sandbox(Host::Claude);
  let files: ConfigFiles = super::config_files(&roots, None);
  user.inspect(|text| place(&files.user, text));
  project.inspect(|text| place(files.project.as_ref().unwrap(), text));
  (dir, load(&roots, None))
}

#[test]
fn no_config_file_is_empty_valid_and_silent() {
  let (dir, config) = loaded(None, None);
  assert!(config.data.is_empty() && config.invalid.is_none());
  assert_eq!(config.take_warnings(), Vec::<String>::new());
  assert_eq!(
    config.files.user,
    dir.path().join("home/.claude/toolu.config.json")
  );
  assert_eq!(
    config.files.project,
    Some(dir.path().join("project/.claude/toolu.config.json"))
  );
  assert_eq!(config.host, Host::Claude);
}

#[test]
fn user_and_project_deep_merge_like_jq() {
  let (_dir, config) = loaded(
    Some(&fixture("edge-values.json")),
    Some(&fixture("merge-project.json")),
  );
  assert_eq!(config.take_warnings(), Vec::<String>::new());
  let ts = &config.data["lang"]["ts"];
  assert_eq!(
    ts,
    &json!({ "maxFileLines": "120", "maxFnLines": 42, "noMocks": "false" })
  );
  assert_eq!(config.data["docsSync"]["surfaces"], json!(["only.md"]));
  assert_eq!(config.data["docsSync"]["mode"], json!("block"));
  assert_eq!(
    config.data["gates"]["pushReview"],
    json!({ "mode": "block" })
  );
  assert_eq!(config.data["gates"]["sweep"], json!(false));
}

#[test]
fn merge_replaces_non_objects_from_the_project_side() {
  assert_eq!(
    merge(&json!({ "a": { "b": 1 } }), &json!({ "a": null })),
    json!({ "a": null })
  );
  assert_eq!(
    merge(&json!({ "a": 1 }), &json!({ "a": { "b": 1 } })),
    json!({ "a": { "b": 1 } })
  );
  assert_eq!(
    merge(&json!({ "a": [1, 2] }), &json!({ "a": [3] })),
    json!({ "a": [3] })
  );
  let merged = merge(
    &json!({ "a": { "b": 1, "c": 2 } }),
    &json!({ "a": { "c": 3 } }),
  );
  assert_eq!(merged, json!({ "a": { "b": 1, "c": 3 } }));
}

#[test]
fn a_malformed_project_file_is_ignored_with_one_warning() {
  for text in ["{\"gates\":", "", "null", "false", "{\"a\":\"\\ud800\"}"] {
    let (_dir, config) = loaded(Some(&fixture("docs-strict.json")), Some(text));
    assert_eq!(config.invalid, None, "{text}");
    assert_eq!(config.data["gates"], json!({ "preset": "strict" }));
    let project = config.files.project.as_ref().unwrap().display().to_string();
    assert_eq!(
      config.take_warnings(),
      [format!("malformed JSON in {project}; ignoring")]
    );
  }
}

#[test]
fn a_rejected_envelope_fails_closed_and_names_the_file() {
  let cases = [
    (
      "fail-closed-unknown-key.json",
      "unknown top-level key 'nope'",
    ),
    (
      "fail-closed-version-2.json",
      "unsupported version 2 (supported: 1)",
    ),
    ("fail-closed-array.json", "top level is not a JSON object"),
  ];
  for (name, reason) in cases {
    let (_dir, config) = loaded(Some(&fixture("docs-strict.json")), Some(&fixture(name)));
    let project = config.files.project.as_ref().unwrap().display().to_string();
    let invalid = format!("{project}: {reason}");
    assert_eq!(config.invalid.as_deref(), Some(invalid.as_str()));
    assert!(config.data.is_empty());
    assert_eq!(
      config.take_warnings(),
      [format!("{invalid}; failing closed (every gate blocks)")]
    );
  }
}

#[test]
fn unknown_keys_are_listed_in_document_order() {
  let (_dir, config) = loaded(None, Some(r#"{"zeta":1,"version":1,"alpha":2,"zeta":3}"#));
  assert!(
    config
      .invalid
      .unwrap()
      .ends_with(": unknown top-level keys 'zeta', 'alpha'")
  );
}

#[test]
fn versions_compare_numerically_and_print_as_javascript_does() {
  let cases = [
    ("\"1\"", Some("\"1\"")),
    ("1.0", None),
    ("2.50", Some("2.5")),
    ("null", Some("null")),
    ("1", None),
  ];
  for (version, shown) in cases {
    let (_dir, config) = loaded(None, Some(&format!("{{\"version\":{version}}}")));
    let reason = config
      .invalid
      .as_deref()
      .and_then(|text| text.rsplit_once(".json: "))
      .map(|(_, reason)| reason.to_owned());
    let expected = shown.map(|shown| format!("unsupported version {shown} (supported: 1)"));
    assert_eq!(reason, expected, "{version}");
  }
}

#[test]
fn an_epic_section_is_never_inspected_but_a_sibling_unknown_key_fails() {
  let (_dir, open) = loaded(
    None,
    Some(r#"{"version":1,"epic":{"futureKnob":true,"nested":{"x":1}}}"#),
  );
  assert_eq!(open.invalid, None);
  assert_eq!(open.data["epic"]["futureKnob"], Value::Bool(true));
  let (_dir, closed) = loaded(None, Some(r#"{"version":1,"epic":{},"nope":1}"#));
  assert!(
    closed
      .invalid
      .unwrap()
      .ends_with(": unknown top-level key 'nope'")
  );
}

#[test]
fn the_user_reason_wins_when_both_files_are_invalid() {
  let (_dir, config) = loaded(Some("[]"), Some(&fixture("fail-closed-unknown-key.json")));
  assert!(
    config
      .invalid
      .as_deref()
      .unwrap()
      .ends_with("home/.claude/toolu.config.json: top level is not a JSON object")
  );
  assert_eq!(config.take_warnings().len(), 2);
}

#[test]
fn a_directory_at_the_config_path_is_absent_and_invalid_utf8_decodes_lossily() {
  let (_dir, roots) = sandbox(Host::Claude);
  let files = super::config_files(&roots, None);
  std::fs::create_dir_all(files.project.as_ref().unwrap()).unwrap();
  let config = load(&roots, None);
  assert!(config.data.is_empty() && config.take_warnings().is_empty());
  std::fs::create_dir_all(files.user.parent().unwrap()).unwrap();
  std::fs::write(&files.user, b"{\"skills\":{\"a\xff\":true}}").unwrap();
  assert_eq!(
    load(&roots, None).data["skills"],
    json!({ "a\u{fffd}": true })
  );
}

#[test]
fn codex_and_toolu_user_config_dir_choose_their_files() {
  let (dir, roots) = sandbox(Host::Codex);
  let files = super::config_files(&roots, None);
  assert_eq!(files.user, dir.path().join("home/.codex/toolu.config.json"));
  assert_eq!(
    files.project,
    Some(dir.path().join("project/.codex/toolu.config.json"))
  );
  let global = dir.path().join("global");
  let env = roots
    .env()
    .clone()
    .with("TOOLU_USER_CONFIG_DIR", &global.display().to_string())
    .with("TOOLU_CONFIG_DIR", "/data");
  let opencode = Roots::new(env, Some(Host::Opencode));
  assert_eq!(
    super::config_files(&opencode, None).user,
    global.join("toolu.config.json")
  );
}

#[test]
fn warnings_are_printed_after_the_config_prefix() {
  assert_eq!(WARN_PREFIX, "toolu-config: ");
}

#[test]
fn check_text_matches_the_loader_rule() {
  assert_eq!(
    check_text(r#"{"bogus":1}"#).unwrap_err(),
    "unknown top-level key 'bogus'"
  );
  assert_eq!(
    check_text(r#"{"version":2}"#).unwrap_err(),
    "unsupported version 2 (supported: 1)"
  );
  assert_eq!(
    check_text("[1]").unwrap_err(),
    "top level is not a JSON object"
  );
  assert_eq!(
    check_text("true").unwrap_err(),
    "top level is not a JSON object"
  );
  for text in ["{", "", "null", "false"] {
    assert_eq!(check_text(text).unwrap_err(), "malformed JSON", "{text}");
  }
  let ok = check_text(r#"{"version":1,"gates":{"pushReview":"off"}}"#).unwrap();
  assert_eq!(ok["gates"]["pushReview"], json!("off"));
}

#[test]
fn exists_is_a_stat_of_either_file() {
  let (_dir, roots) = sandbox(Host::Claude);
  assert!(!exists(&roots, None));
  let files = super::config_files(&roots, None);
  place(files.project.as_ref().unwrap(), "{\"version\":1}");
  assert!(exists(&roots, None));
  let codex = Roots::new(roots.env().clone(), Some(Host::Codex));
  assert!(!exists(&codex, None));
  place(&super::config_files(&codex, None).user, "{}");
  assert!(exists(&codex, None));
}
