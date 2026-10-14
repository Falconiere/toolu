//! The lifecycle case contract (`fixtures/gates/lifecycle.json`) and its golden captures
//! (`fixtures/gates/lifecycle-golden.json`), read the way the retired TypeScript suites did.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// One sandbox and the hook it runs.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Case {
  pub(crate) name: String,
  pub(crate) hook: String,
  pub(crate) stdin: String,
  pub(crate) host: Option<String>,
  pub(crate) git: Option<bool>,
  pub(crate) branch: Option<String>,
  pub(crate) subdir: Option<String>,
  pub(crate) files: Option<BTreeMap<String, String>>,
  pub(crate) untracked: Option<BTreeMap<String, String>>,
  pub(crate) user_config: Option<Value>,
  pub(crate) registry: Option<Value>,
  pub(crate) wrappers: Option<Vec<String>>,
  pub(crate) manifest: Option<Value>,
  pub(crate) codex_manifest: Option<Value>,
  pub(crate) codex_list: Option<String>,
  pub(crate) ast_grep: Option<bool>,
  pub(crate) first_run: Option<bool>,
  pub(crate) env: Option<BTreeMap<String, String>>,
}

/// What a hook run printed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Captured {
  pub(crate) stdout: String,
  pub(crate) stderr: String,
  pub(crate) exit_code: i32,
}

/// The captures and the bash commit they came from.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Golden {
  pub(crate) base: String,
  pub(crate) cases: BTreeMap<String, Captured>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
  version: u32,
  cases: Vec<Case>,
}

/// The repository root, canonical like the sandbox paths.
pub(crate) fn repo_root() -> Res<PathBuf> {
  Ok(
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../..")
      .canonicalize()?,
  )
}

/// Every case of the shared contract, names unique.
pub(crate) fn read_cases(root: &Path) -> Res<Vec<Case>> {
  let text = std::fs::read_to_string(root.join("fixtures/gates/lifecycle.json"))?;
  let file: CaseFile = serde_json::from_str(&text)?;
  if file.version != 1 {
    return Err(format!("lifecycle.json version {}", file.version).into());
  }
  let mut seen = BTreeSet::new();
  for case in &file.cases {
    if !seen.insert(case.name.clone()) {
      return Err(format!("duplicate case name: {}", case.name).into());
    }
  }
  Ok(file.cases)
}

/// The captures, with exactly one per case.
pub(crate) fn read_golden(root: &Path, cases: &[Case]) -> Res<Golden> {
  let text = std::fs::read_to_string(root.join("fixtures/gates/lifecycle-golden.json"))?;
  let golden: Golden = serde_json::from_str(&text)?;
  let want: BTreeSet<&str> = cases.iter().map(|case| case.name.as_str()).collect();
  let have: BTreeSet<&str> = golden.cases.keys().map(String::as_str).collect();
  if want != have {
    return Err("capture names differ from case names".into());
  }
  Ok(golden)
}
