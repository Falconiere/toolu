//! No exemption for `crates/` exists anywhere (#455): the gate data carries no
//! ignore, skip or per-path override, and no crate source holds a suppression,
//! a mod file, a build script or a source include.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn Error>>;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Lint levels `[workspace.lints]` may set to `allow`: rules for every crate.
const RELAXED: &[&str] = &["module_name_repetitions", "must_use_candidate"];

fn toml_at(root: &Path, file: &str) -> Res<toml::Table> {
  Ok(fs::read_to_string(root.join(file))?.parse()?)
}

/// Every exemption the gate data of `root` grants.
fn exemptions(root: &Path) -> Res<Vec<String>> {
  let mut found = Vec::new();
  let deny = toml_at(root, "deny.toml")?;
  for (section, keys) in [
    ("advisories", &["ignore"][..]),
    ("bans", &["skip", "skip-tree", "allow-wildcard-paths"][..]),
    ("licenses", &["exceptions", "clarify"][..]),
    ("sources", &["allow-git", "allow-org"][..]),
  ] {
    let table = deny.get(section).and_then(toml::Value::as_table);
    for key in keys {
      if table.is_some_and(|table| table.contains_key(*key)) {
        found.push(format!("deny.toml [{section}] {key}"));
      }
    }
  }
  let jscpd: serde_json::Value = serde_json::from_str(&fs::read_to_string(
    root.join("tooling/conventions/guardrails/rust/jscpd.json"),
  )?)?;
  for key in ["ignore", "ignorePattern"] {
    if jscpd.get(key).is_some() {
      found.push(format!("jscpd.json {key}"));
    }
  }
  for key in toml_at(root, "clippy.toml")?.keys() {
    if key.starts_with("allowed-") || key.starts_with("disallowed-") {
      found.push(format!("clippy.toml {key}"));
    }
  }
  found.extend(relaxed_lints(&toml_at(root, "Cargo.toml")?));
  Ok(found)
}

/// Lints set to `allow` beyond the documented relaxations.
fn relaxed_lints(manifest: &toml::Table) -> Vec<String> {
  let lints = manifest
    .get("workspace")
    .and_then(|workspace| workspace.get("lints"))
    .and_then(toml::Value::as_table);
  let mut found = Vec::new();
  for (tool, table) in lints.into_iter().flatten() {
    for (lint, level) in table.as_table().into_iter().flatten() {
      let level = level
        .as_str()
        .or_else(|| level.get("level").and_then(toml::Value::as_str));
      if level == Some("allow") && !RELAXED.contains(&lint.as_str()) {
        found.push(format!("[workspace.lints.{tool}] {lint} = allow"));
      }
    }
  }
  found
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) -> Res<()> {
  for entry in fs::read_dir(dir)? {
    let path = entry?.path();
    if path.is_dir() {
      files(&path, out)?;
    } else {
      out.push(path);
    }
  }
  Ok(())
}

/// Suppressions, mod files, build scripts and includes under `crates/` of `root`.
fn banned_in_crates(root: &Path) -> Res<Vec<String>> {
  let literals = [
    ["#[", "allow"].concat(),
    ["#[", "expect"].concat(),
    ["#![", "allow"].concat(),
    ["include", "!"].concat(),
  ];
  let names = [["mod", ".rs"].concat(), ["build", ".rs"].concat()];
  let literals: Vec<String> = literals.into_iter().chain(names.iter().cloned()).collect();
  let mut all = Vec::new();
  files(&root.join("crates"), &mut all)?;
  let mut found = Vec::new();
  for path in all {
    let name = path
      .file_name()
      .map(|name| name.to_string_lossy().into_owned());
    if name.is_some_and(|name| names.contains(&name)) {
      found.push(path.display().to_string());
    }
    let Ok(text) = fs::read_to_string(&path) else {
      continue;
    };
    for literal in &literals {
      if text.contains(literal.as_str()) {
        found.push(format!("{}: {literal}", path.display()));
      }
    }
  }
  Ok(found)
}

#[test]
fn the_repository_grants_no_exemption() {
  assert_eq!(exemptions(Path::new(REPO)).unwrap(), Vec::<String>::new());
}

#[test]
fn the_crates_hold_no_banned_construct() {
  assert_eq!(
    banned_in_crates(Path::new(REPO)).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn an_added_exemption_is_found() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  for file in [
    "deny.toml",
    "clippy.toml",
    "Cargo.toml",
    "tooling/conventions/guardrails/rust/jscpd.json",
  ] {
    fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
    fs::copy(Path::new(REPO).join(file), root.join(file)).unwrap();
  }
  let deny = fs::read_to_string(root.join("deny.toml")).unwrap();
  fs::write(
    root.join("deny.toml"),
    deny.replace("wildcards = \"deny\"", "wildcards = \"deny\"\nskip = []"),
  )
  .unwrap();
  let clippy = fs::read_to_string(root.join("clippy.toml")).unwrap();
  fs::write(
    root.join("clippy.toml"),
    format!("{clippy}disallowed-names = []\n"),
  )
  .unwrap();
  let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
  fs::write(
    root.join("Cargo.toml"),
    manifest.replace("unwrap_used = \"deny\"", "unwrap_used = \"allow\""),
  )
  .unwrap();
  assert_eq!(
    exemptions(root).unwrap(),
    [
      "deny.toml [bans] skip",
      "clippy.toml disallowed-names",
      "[workspace.lints.clippy] unwrap_used = allow"
    ]
  );
}

#[test]
fn an_added_banned_construct_is_found() {
  let dir = tempfile::tempdir().unwrap();
  let src = dir.path().join("crates/demo/src");
  fs::create_dir_all(&src).unwrap();
  fs::write(src.join(["mod", ".rs"].concat()), "").unwrap();
  fs::write(
    src.join("lib.rs"),
    ["#[", "allow(dead_code)]\nfn f() {}\n"].concat(),
  )
  .unwrap();
  let found = banned_in_crates(dir.path()).unwrap();
  assert_eq!(found.len(), 2, "{found:?}");
  fs::write(src.join("lib.rs"), ["// see ", "build", ".rs\n"].concat()).unwrap();
  assert_eq!(banned_in_crates(dir.path()).unwrap().len(), 2);
}
