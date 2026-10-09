//! `cargo xtask pack`: published tarball file lists.

use std::path::Path;

use serde_json::Value;

use crate::options::Options;
use crate::pack_closure::closure_problems;
use crate::pack_npm;
use crate::pack_scan::{self, has_ext};
use crate::{Verdict, output};

struct Expectation {
  name: &'static str,
  dir: &'static str,
  required: Vec<String>,
  forbidden: &'static [&'static str],
  patterns: &'static [fn(&str) -> bool],
  exact: bool,
}

const PLUGINS_PATTERNS: &[fn(&str) -> bool] = &[hook_source];
const OPENCODE_PATTERNS: &[fn(&str) -> bool] = &[hook_source, test_file, shell_file];

/// Pack each published package and apply its required and forbidden lists.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let mut problems = Vec::new();
  for expectation in expectations(&options.root)? {
    if expectation.name == "@toolu/opencode" {
      problems.extend(opencode(&options.root, &expectation)?);
      continue;
    }
    let files = pack_npm::packed_files(&options.root.join(expectation.dir))?;
    output::say(&format!(
      "pack-inventory: {} — {} files",
      expectation.name,
      files.len()
    ));
    let paths = files
      .iter()
      .map(|file| file.path.clone())
      .collect::<Vec<_>>();
    problems.extend(check_one(&expectation, &paths));
  }
  Ok(report(&problems))
}

fn opencode(root: &Path, expectation: &Expectation) -> Result<Vec<String>, String> {
  let stage = pack_npm::Stage::opencode(root)?;
  let files = pack_npm::packed_files(&stage.package)?;
  output::say(&format!(
    "pack-inventory: {} — {} files",
    expectation.name,
    files.len()
  ));
  let paths = files
    .iter()
    .map(|file| file.path.clone())
    .collect::<Vec<_>>();
  let mut problems = check_one(expectation, &paths);
  problems.extend(closure_problems(
    &stage.package,
    &files,
    &root.join("plugins"),
    &core_exports(root)?,
  )?);
  Ok(problems)
}

fn report(problems: &[String]) -> Verdict {
  for problem in problems {
    output::error(&format!("RED  {problem}"));
  }
  if problems.is_empty() {
    output::say("pack-inventory: ok");
    return Verdict::Clean;
  }
  Verdict::Findings
}

fn expectations(root: &Path) -> Result<Vec<Expectation>, String> {
  Ok(vec![
    plugins_package(),
    core_package(),
    opencode_package(root)?,
  ])
}

fn plugins_package() -> Expectation {
  Expectation {
    name: "@toolu/plugins",
    dir: "tools/toolu-cli/npm",
    required: owned(&[
      "package.json",
      "README.md",
      "LICENSE",
      "assets/marketplace.json",
      "dist/cli.js",
    ]),
    forbidden: &["plugins/", "src/", "node_modules/", ".env"],
    patterns: PLUGINS_PATTERNS,
    exact: true,
  }
}

fn core_package() -> Expectation {
  Expectation {
    name: "@toolu/core",
    dir: "packages/toolu-core",
    required: owned(&[
      "package.json",
      "README.md",
      "LICENSE",
      "src/decision/decision.ts",
      "src/dispatch/dispatch.ts",
    ]),
    forbidden: &["plugins/", "dist/", "node_modules/", ".env"],
    patterns: &[],
    exact: false,
  }
}

fn opencode_package(root: &Path) -> Result<Expectation, String> {
  let mut required = owned(&[
    "package.json",
    "README.md",
    "LICENSE",
    "src/plugin/toolu.ts",
    "src/bootstrap/native-launcher.txt",
    "plugins/toolu/hooks/hooks.json",
    "plugins/rust-quality/.claude-plugin/plugin.json",
    "plugins/toolu/settings/protected-files.txt",
    "plugins/epic-orchestrator/scripts/launch-issue.ts",
    "plugins/epic-orchestrator/scripts/epic-watch.ts",
    "plugins/epic-orchestrator/scripts/trackers/jira.ts",
    "plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md",
    "plugins/pr-babysit/skills/babysit/references/fixer-brief.md",
    "plugins/jev/scripts/jev.sh",
  ]);
  required.extend(committed_bundles(root)?);
  Ok(Expectation {
    name: "@toolu/opencode",
    dir: "tools/toolu-opencode",
    required,
    forbidden: &["node_modules/", ".env"],
    patterns: OPENCODE_PATTERNS,
    exact: false,
  })
}

fn check_one(expectation: &Expectation, files: &[String]) -> Vec<String> {
  let mut problems = Vec::new();
  for required in &expectation.required {
    if !files.iter().any(|file| file == required) {
      problems.push(format!(
        "{} tarball is missing {required}",
        expectation.name
      ));
    }
  }
  for forbidden in expectation.forbidden {
    for file in files {
      if file.starts_with(forbidden) {
        problems.push(format!(
          "{} tarball must not contain {file}",
          expectation.name
        ));
      }
    }
  }
  for pattern in expectation.patterns {
    for file in files {
      if pattern(file) {
        problems.push(format!(
          "{} tarball must not contain {file}",
          expectation.name
        ));
      }
    }
  }
  if expectation.exact {
    for file in files {
      if !expectation.required.iter().any(|required| required == file) {
        problems.push(format!(
          "{} tarball has an undeclared file: {file}",
          expectation.name
        ));
      }
    }
  }
  pack_scan::unique(problems)
}

fn hook_source(path: &str) -> bool {
  path.starts_with("hooks/src/") || path.contains("/hooks/src/")
}

fn test_file(path: &str) -> bool {
  path.starts_with("__tests__/")
    || path.starts_with("fixtures/")
    || path.contains("/__tests__/")
    || path.contains("/fixtures/")
}

fn shell_file(path: &str) -> bool {
  if path == "plugins/jev/scripts/jev.sh" {
    return false;
  }
  has_ext(path, "sh") || has_ext(path, "bash") || has_ext(path, "bats")
}

fn committed_bundles(root: &Path) -> Result<Vec<String>, String> {
  let mut bundles = Vec::new();
  for plugin in dir_names(&root.join("plugins"))? {
    for file in file_names(&root.join("plugins").join(&plugin).join("hooks/dist"))? {
      if has_ext(&file, "js") {
        bundles.push(format!("plugins/{plugin}/hooks/dist/{file}"));
      }
    }
  }
  Ok(bundles)
}

fn dir_names(dir: &Path) -> Result<Vec<String>, String> {
  names(dir, true)
}

fn file_names(dir: &Path) -> Result<Vec<String>, String> {
  if !dir.is_dir() {
    return Ok(Vec::new());
  }
  names(dir, false)
}

fn names(dir: &Path, directories: bool) -> Result<Vec<String>, String> {
  let mut found = Vec::new();
  for entry in
    std::fs::read_dir(dir).map_err(|err| format!("pack: cannot list {}: {err}", dir.display()))?
  {
    let entry = entry.map_err(|err| format!("pack: cannot list {}: {err}", dir.display()))?;
    let meta = entry
      .metadata()
      .map_err(|err| format!("pack: cannot list {}: {err}", dir.display()))?;
    if meta.is_dir() == directories {
      found.push(entry.file_name().to_string_lossy().into_owned());
    }
  }
  found.sort();
  Ok(found)
}

fn core_exports(root: &Path) -> Result<Vec<String>, String> {
  let path = root.join("packages/toolu-core/package.json");
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("pack: cannot read {}: {err}", path.display()))?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("pack: {}: {err}", path.display()))?;
  let exports = doc
    .get("exports")
    .and_then(Value::as_object)
    .ok_or_else(|| "pack: @toolu/core package.json has no exports".to_owned())?;
  Ok(exports.keys().cloned().collect())
}

fn owned(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

#[cfg(test)]
#[path = "tests/pack_test.rs"]
mod tests;
