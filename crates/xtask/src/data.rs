//! The quality-bar data under `tooling/conventions/guardrails/rust/` and the
//! `lang.rust` limits. Every shape rejects unknown keys, so an `ignore`,
//! `exempt` or per-path override cannot be added without changing this code.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Where the data lives, relative to the workspace root.
pub(crate) const DATA_DIR: &str = "tooling/conventions/guardrails/rust";

/// The repository config that holds `lang.rust`.
pub(crate) const CONFIG: &str = ".claude/toolu.config.json";

/// `rules.json`: gate data.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Rules {
  pub(crate) version: u32,
  pub(crate) tests: TestRules,
  pub(crate) structure: StructureRules,
  pub(crate) capabilities: Vec<Capability>,
  pub(crate) capability_crates: Vec<CapabilityCrates>,
  pub(crate) secrets: Vec<Secret>,
  pub(crate) inventory: InventoryRules,
  pub(crate) coverage: CoverageRules,
}

/// Test layout (rules 6 and 7).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TestRules {
  /// Subdirectories allowed inside a `tests/` directory.
  pub(crate) subdirs: Vec<String>,
  /// The suffix of a unit-test file stem (`_test`).
  pub(crate) file_suffix: String,
}

/// Folder shapes (rule 11).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StructureRules {
  /// Directory levels allowed under `src`.
  pub(crate) max_src_depth: usize,
  /// Crate directory names that may hold a `main.rs`.
  pub(crate) main_crates: Vec<String>,
  /// Entries allowed in a crate directory.
  #[serde(rename = "crate")]
  pub(crate) krate: Vec<String>,
  /// Entries allowed in a crate's admitted `fuzz/` package.
  pub(crate) fuzz: Vec<String>,
  /// Entries allowed in a `plugins/<name>/` directory.
  pub(crate) plugin: Vec<String>,
}

/// A capability only its owners may use (rule 14).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Capability {
  pub(crate) id: String,
  /// Full paths such as `std::env::var`.
  pub(crate) paths: Vec<String>,
  pub(crate) owners: Vec<Owner>,
}

/// A crate, or one top-level module of it, that owns a capability.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Owner {
  #[serde(rename = "crate")]
  pub(crate) krate: String,
  pub(crate) module: Option<String>,
}

/// Crates only one workspace member may link (rule 14).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilityCrates {
  pub(crate) id: String,
  pub(crate) crates: Vec<String>,
  pub(crate) owner: String,
}

/// A secret pattern (rule 23).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Secret {
  pub(crate) id: String,
  pub(crate) regex: String,
}

/// Behaviour-inventory kinds and where their items are discovered (rule 8).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InventoryRules {
  pub(crate) kinds: Vec<InventoryKind>,
}

/// One kind: the string literals of `constant` in `file` are its items.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InventoryKind {
  pub(crate) kind: String,
  pub(crate) file: String,
  #[serde(rename = "const")]
  pub(crate) constant: String,
}

/// Coverage floors in percent of lines (rule 9).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageRules {
  pub(crate) default: f64,
  pub(crate) strict: BTreeMap<String, f64>,
}

/// `folders.json`: registration data for the folder allowlist.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Folders {
  /// Entries allowed at the repository root.
  pub(crate) root: Vec<String>,
  /// Entries allowed in `crates/`.
  pub(crate) crates: Vec<String>,
  /// Entries allowed in `crates/core/`.
  pub(crate) core: Vec<String>,
}

/// `inventory.json`: registration data.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
  pub(crate) entries: Vec<InventoryEntry>,
}

/// One item with its passing and failing scenario, each `<path>::<test fn>`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InventoryEntry {
  pub(crate) kind: String,
  pub(crate) id: String,
  pub(crate) pass: String,
  pub(crate) fail: String,
}

/// `coverage-floor.json`: registration data, raised per crate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoverageFloor {
  pub(crate) floors: BTreeMap<String, f64>,
}

/// `lang.rust` of the repository config. Its other keys belong to the
/// rust-quality plugin's schema, which validates them.
#[derive(Debug, Deserialize)]
pub(crate) struct Limits {
  /// `maxFileLines`: code lines per file.
  #[serde(rename = "maxFileLines")]
  pub(crate) file: usize,
  /// `maxFnLines`: code lines per function (clippy's `too-many-lines-threshold`).
  #[serde(rename = "maxFnLines")]
  pub(crate) function: usize,
  /// `maxImplLines`: code lines per `impl` block.
  #[serde(rename = "maxImplLines")]
  pub(crate) impl_block: usize,
}

/// Read and parse `DATA_DIR/name` under `root`.
pub(crate) fn load<T: DeserializeOwned>(root: &Path, name: &str) -> Result<T, String> {
  let path = Path::new(DATA_DIR).join(name);
  let text = std::fs::read_to_string(root.join(&path))
    .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
  serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// `rules.json`, with its version checked.
pub(crate) fn rules(root: &Path) -> Result<Rules, String> {
  let rules: Rules = load(root, "rules.json")?;
  if rules.version != 1 {
    return Err(format!(
      "{DATA_DIR}/rules.json: unsupported version {}",
      rules.version
    ));
  }
  Ok(rules)
}

/// `lang.rust` from `CONFIG` under `root`; absent limits fail closed.
pub(crate) fn limits(root: &Path) -> Result<Limits, String> {
  limits_at(root, Path::new(CONFIG))
}

/// `lang.rust` from the toolu config `file` under `root`.
pub(crate) fn limits_at(root: &Path, file: &Path) -> Result<Limits, String> {
  let shown = file.display();
  let text = std::fs::read_to_string(root.join(file))
    .map_err(|err| format!("cannot read {shown}: {err}"))?;
  let config: serde_json::Value =
    serde_json::from_str(&text).map_err(|err| format!("{shown}: {err}"))?;
  let rust = config
    .get("lang")
    .and_then(|lang| lang.get("rust"))
    .ok_or_else(|| format!("{shown} has no lang.rust limits"))?;
  Limits::deserialize(rust).map_err(|err| format!("{shown} lang.rust: {err}"))
}

#[cfg(test)]
#[path = "tests/data_test.rs"]
mod tests;
