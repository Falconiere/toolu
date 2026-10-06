use std::path::{Path, PathBuf};

use super::{ModuleManifest, read_manifest};
use crate::registry::RegistryEvent;

fn place(dir: &Path, base: &str, text: &str) -> PathBuf {
  let path = dir.join(base);
  std::fs::write(&path, text).unwrap();
  path
}

const VALID: &str = r#"{"version":1,"spec":"ts-quality@toolu","name":"max-lines","event":"tool/post","matcher":"Edit|Write|mcp__*"}"#;

#[test]
fn a_valid_manifest_reads_and_round_trips() {
  let dir = tempfile::tempdir().unwrap();
  let path = place(dir.path(), "ts-quality@toolu__max-lines.json", VALID);
  let manifest = read_manifest(&path, RegistryEvent::ToolPost).unwrap();
  assert_eq!(manifest.event, RegistryEvent::ToolPost);
  assert_eq!(serde_json::to_string(&manifest).unwrap(), VALID);
}

#[test]
fn the_matcher_selects_exactly_the_listed_tools() {
  let manifest: ModuleManifest = serde_json::from_str(VALID).unwrap();
  let picked: Vec<&str> = ["Edit", "Write", "mcp__github__x", "Bash", "EditX", "mcp_"]
    .into_iter()
    .filter(|tool| manifest.matches(tool))
    .collect();
  assert_eq!(picked, ["Edit", "Write", "mcp__github__x"]);
  let every = ModuleManifest {
    matcher: "*".to_owned(),
    ..manifest
  };
  assert!(every.matches("Anything") && every.matches(""));
}

fn rejects(cases: &[(&str, String, &str)]) {
  let dir = tempfile::tempdir().unwrap();
  for (name, text, reason) in cases {
    let path = place(dir.path(), name, text);
    let error = read_manifest(&path, RegistryEvent::ToolPost).unwrap_err();
    let named = error.starts_with(&path.display().to_string());
    assert!(named && error.contains(reason), "{name} {text}: {error}");
  }
}

const BASE: &str = "ts-quality@toolu__max-lines.json";

#[test]
fn a_manifest_off_schema_or_off_its_file_name_is_rejected() {
  rejects(&[
    (
      BASE,
      VALID.replace("\"matcher\"", "\"extra\":1,\"matcher\""),
      "unknown field `extra`",
    ),
    (
      BASE,
      VALID.replace("\"version\":1", "\"version\":2"),
      "unsupported version 2",
    ),
    (
      "ts-quality@toolu__other.json",
      VALID.to_owned(),
      "not the file's ts-quality@toolu__other",
    ),
    (
      BASE,
      VALID.replace("tool/post", "tool/pre"),
      "event tool/pre in the post-tools.d directory",
    ),
    (
      "max-lines.json",
      VALID.to_owned(),
      "not a <spec>__<name>.json file",
    ),
    (
      "ts-quality@toolu__max-lines.js",
      VALID.to_owned(),
      "not a <spec>__<name>.json file",
    ),
  ]);
}

#[test]
fn a_bad_matcher_or_unreadable_text_is_rejected() {
  rejects(&[
    (
      BASE,
      VALID.replace("Edit|Write|mcp__*", "Edit||Write"),
      "an empty matcher entry",
    ),
    (
      BASE,
      VALID.replace("Edit|Write|mcp__*", ""),
      "an empty matcher entry",
    ),
    (BASE, "{".to_owned(), "EOF while parsing"),
  ]);
  let dir = tempfile::tempdir().unwrap();
  let missing =
    read_manifest(&dir.path().join("absent__x.json"), RegistryEvent::ToolPre).unwrap_err();
  assert!(missing.contains("absent__x.json"));
}
