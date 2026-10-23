//! `cargo xtask pack`: published tarball file lists.

use std::collections::HashSet;

use crate::options::Options;
use crate::pack_npm;
use crate::{Verdict, output};

struct Expectation {
  name: &'static str,
  dir: &'static str,
  required: Vec<String>,
  /// Shipped files that may be absent (staged only at release time).
  optional: Vec<String>,
  forbidden: &'static [&'static str],
  patterns: &'static [fn(&str) -> bool],
  exact: bool,
}

const PLUGINS_PATTERNS: &[fn(&str) -> bool] = &[hook_source];

/// Pack each published package and apply its required and forbidden lists.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let mut problems = Vec::new();
  for expectation in expectations() {
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

fn expectations() -> Vec<Expectation> {
  vec![plugins_package()]
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
      "cli.js",
    ]),
    optional: owned(&[
      "bin/darwin-arm64/toolu",
      "bin/darwin-amd64/toolu",
      "bin/linux-amd64/toolu",
      "bin/linux-arm64/toolu",
    ]),
    forbidden: &["plugins/", "src/", "node_modules/", ".env"],
    patterns: PLUGINS_PATTERNS,
    exact: true,
  }
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
      let known = expectation.required.iter().any(|required| required == file)
        || expectation.optional.iter().any(|allowed| allowed == file);
      if !known {
        problems.push(format!(
          "{} tarball has an undeclared file: {file}",
          expectation.name
        ));
      }
    }
  }
  unique(problems)
}

fn hook_source(path: &str) -> bool {
  path.starts_with("hooks/src/") || path.contains("/hooks/src/")
}

fn unique(items: Vec<String>) -> Vec<String> {
  let mut seen = HashSet::new();
  items
    .into_iter()
    .filter(|item| seen.insert(item.clone()))
    .collect()
}

fn owned(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

#[cfg(test)]
#[path = "tests/pack_test.rs"]
mod tests;
