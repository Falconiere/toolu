//! `cargo xtask check-workflows`: the GitHub workflows against the Rust CI and
//! release contract, one finding per broken invariant (#440 ported it from
//! `tooling/src/check-workflows.ts`).

use std::path::Path;

use serde_json::Value;

use crate::ci_yaml::yaml_to_json;
use crate::options::Options;
use crate::workflow_ci::{Docs, actions_and_permissions, aggregates, review, rust_legs, triggers};
use crate::workflow_install::install_channels;
use crate::workflow_release::release;
use crate::workflow_yaml::Findings;
use crate::{Verdict, output};

/// The workflows the contract reads.
const NAMES: [&str; 8] = [
  "tests.yml",
  "toolu-review.yml",
  "release-please.yml",
  "release-native.yml",
  "release-homebrew.yml",
  "release-finalize.yml",
  "advisory-audit.yml",
  "npm-publish.yml",
];

/// Judge the workflows under `--root`.
///
/// # Errors
/// When `release-please-config.json` cannot be read or parsed.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let found = check(&options.root)?;
  for line in &found {
    output::error(&format!("check-workflows: {line}"));
  }
  if found.is_empty() {
    output::say("check-workflows: workflows match the Rust release contract");
    return Ok(Verdict::Clean);
  }
  Ok(Verdict::Findings)
}

/// One finding per violated invariant, so a mutated fixture names its break.
pub(crate) fn check(root: &Path) -> Result<Vec<String>, String> {
  let mut found = Findings::default();
  let mut parsed = Vec::new();
  for name in NAMES {
    match read(root, name) {
      Ok(doc) => {
        actions_and_permissions(name, &doc, &mut found);
        parsed.push((name.to_owned(), doc));
      }
      Err(err) => found.0.push(format!("{name}: cannot read or parse: {err}")),
    }
  }
  let docs = Docs(&parsed);
  triggers(&docs, &mut found);
  aggregates(docs.get("tests.yml"), &mut found);
  rust_legs(docs.get("tests.yml"), &mut found);
  review(root, docs.get("toolu-review.yml"), &mut found);
  release(root, &docs, &mut found)?;
  install_channels(root, &docs, &mut found);
  Ok(found.0)
}

fn read(root: &Path, name: &str) -> Result<Value, String> {
  let path = root.join(".github/workflows").join(name);
  let text = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
  yaml_to_json(&text)
}

#[cfg(test)]
#[path = "tests/check_workflows_test.rs"]
mod tests;
