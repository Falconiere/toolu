//! The plugin inventory (AC-3): the plugin manifests, the plugin crates and the
//! namespace owners in `toolu commands --json` name the same plugins.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cli::{Res, one_document, toolu};

/// The repository this test crate lives in.
fn root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The directories of `parent` that contain `marker`.
fn dirs_with(parent: &Path, marker: &str) -> Res<BTreeSet<String>> {
  let mut names = BTreeSet::new();
  for entry in std::fs::read_dir(parent)? {
    let entry = entry?;
    if entry.path().join(marker).is_file() {
      names.insert(entry.file_name().to_string_lossy().into_owned());
    }
  }
  Ok(names)
}

/// `plugins/<name>/.claude-plugin/plugin.json`.
fn manifests() -> Res<BTreeSet<String>> {
  dirs_with(&root().join("plugins"), ".claude-plugin/plugin.json")
}

/// `crates/<name>/Cargo.toml`, without the binary and tooling crates of `layers.json`.
fn plugin_crates() -> Res<BTreeSet<String>> {
  let layers: Value = serde_json::from_str(&std::fs::read_to_string(
    root().join("tooling/conventions/guardrails/rust/layers.json"),
  )?)?;
  let mut crates = dirs_with(&root().join("crates"), "Cargo.toml")?;
  for role in ["binary", "tooling"] {
    let name = layers.get(role).and_then(Value::as_str);
    crates.remove(name.ok_or("layers.json lacks a role")?);
  }
  Ok(crates)
}

/// The owners of the top-level commands, without `crates/cli`'s own.
fn owners() -> Res<BTreeSet<String>> {
  let tree = one_document(&toolu(&["commands", "--json"])?)?;
  let commands = tree
    .get("commands")
    .and_then(Value::as_array)
    .ok_or("no commands array")?;
  Ok(
    commands
      .iter()
      .filter_map(|command| command["owner"].as_str())
      .filter(|owner| *owner != "toolu-cli")
      .map(str::to_owned)
      .collect(),
  )
}

/// One line per plugin name that one of the three sides lacks.
fn mismatches(
  manifests: &BTreeSet<String>,
  crates: &BTreeSet<String>,
  owners: &BTreeSet<String>,
) -> Vec<String> {
  let all: BTreeSet<&String> = manifests.iter().chain(crates).chain(owners).collect();
  let sides = [
    ("plugin manifest", manifests),
    ("crate", crates),
    ("CLI namespace", owners),
  ];
  all
    .into_iter()
    .flat_map(|name| {
      sides
        .iter()
        .filter(|(_, side)| !side.contains(name))
        .map(move |(what, _)| format!("{name}: no {what}"))
    })
    .collect()
}

#[test]
fn the_twelve_plugins_have_a_manifest_a_crate_and_a_namespace() {
  let (manifests, crates, owners) = (
    manifests().unwrap(),
    plugin_crates().unwrap(),
    owners().unwrap(),
  );
  assert_eq!(
    mismatches(&manifests, &crates, &owners),
    Vec::<String>::new()
  );
  assert_eq!(manifests.len(), 12, "{manifests:?}");
  assert!(manifests.contains("brainstorm") && manifests.contains("delivery-flow"));
}

#[test]
fn a_side_missing_a_plugin_is_named() {
  let (manifests, crates, owners) = (
    manifests().unwrap(),
    plugin_crates().unwrap(),
    owners().unwrap(),
  );
  let without = |side: &BTreeSet<String>, name: &str| {
    let mut side = side.clone();
    side.remove(name);
    side
  };
  assert_eq!(
    mismatches(&without(&manifests, "jev"), &crates, &owners),
    ["jev: no plugin manifest"]
  );
  assert_eq!(
    mismatches(&manifests, &without(&crates, "brainstorm"), &owners),
    ["brainstorm: no crate"]
  );
  assert_eq!(
    mismatches(&manifests, &crates, &without(&owners, "delivery-flow")),
    ["delivery-flow: no CLI namespace"]
  );
}
