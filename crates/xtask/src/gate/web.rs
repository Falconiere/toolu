//! Quality gate for the dashboard's served HTML, CSS, and browser JavaScript.

mod checks;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::options::Options;
use crate::{Verdict, output};

const ASSETS: &str = "crates/epic-orchestrator/assets/dashboard";
const CONFIG: &str = "tooling/conventions/guardrails/web";

/// Run the web checks on the dashboard assets or a named fixture directory.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  if !options.files.is_empty() && options.only != ["web"] {
    return Err("a web fixture path requires `gate --only web PATH`".to_owned());
  }
  if options.files.len() > 1 {
    return Err("gate --only web takes one fixture directory".to_owned());
  }
  let asset = options
    .files
    .first()
    .map_or_else(|| options.root.join(ASSETS), |path| options.root.join(path));
  if asset.is_symlink() {
    return Err(format!("web: symlink asset directory: {}", asset.display()));
  }
  if !asset.exists() && options.files.is_empty() {
    output::say(&format!(
      "web: {ASSETS} absent; dashboard gate will run when assets land"
    ));
    return Ok(Verdict::Clean);
  }
  if !asset.is_dir() {
    return Err(format!("web asset directory missing: {}", asset.display()));
  }
  check_tools(&options.root)?;
  let findings = checks::check(&options.root, &asset)?;
  for finding in &findings {
    output::error(finding);
  }
  let mut failed = !findings.is_empty();
  let tests = test_files(&asset)?;
  for (name, program, args) in commands(&options.root, &asset, &tests) {
    output::say(&format!("web: {name}"));
    if !command(&options.root, &program, &args)? {
      failed = true;
    }
  }
  Ok(if failed {
    Verdict::Findings
  } else {
    Verdict::Clean
  })
}

fn test_files(asset: &Path) -> Result<Vec<String>, String> {
  let dir = asset.join("tests");
  let entries =
    std::fs::read_dir(&dir).map_err(|err| format!("web: cannot read {}: {err}", dir.display()))?;
  let mut tests = Vec::new();
  for entry in entries {
    let entry = entry.map_err(|err| format!("web: {}: {err}", dir.display()))?;
    let kind = entry
      .file_type()
      .map_err(|err| format!("web: {}: {err}", entry.path().display()))?;
    if kind.is_file() && entry.file_name().to_string_lossy().ends_with(".test.js") {
      tests.push(entry.path().display().to_string());
    }
  }
  tests.sort();
  if tests.is_empty() {
    return Err(format!("web: no tests/*.test.js in {}", dir.display()));
  }
  Ok(tests)
}

fn check_tools(root: &Path) -> Result<(), String> {
  for name in ["oxlint", "oxfmt", "html-validate", "jscpd"] {
    if !root.join("node_modules/.bin").join(name).is_file() {
      return Err(format!(
        "web: {name} missing from node_modules; run `npm ci`"
      ));
    }
  }
  let probe = Command::new("ast-grep")
    .arg("--version")
    .current_dir(root)
    .output()
    .map_err(|err| format!("web: ast-grep is required: {err}"))?;
  if !probe.status.success() {
    return Err("web: ast-grep --version failed".to_owned());
  }
  Ok(())
}

fn commands(
  root: &Path,
  asset: &Path,
  tests: &[String],
) -> Vec<(&'static str, PathBuf, Vec<String>)> {
  let bin = root.join("node_modules/.bin");
  let config = root.join(CONFIG);
  let shown = asset.display().to_string();
  let cfg = |name: &str| config.join(name).display().to_string();
  let mut checks = vec![
    (
      "oxlint",
      bin.join("oxlint"),
      vec![
        "--config".into(),
        cfg("oxlintrc.json"),
        "--no-error-on-unmatched-pattern".into(),
        shown.clone(),
      ],
    ),
    (
      "oxfmt",
      bin.join("oxfmt"),
      vec![
        "--check".into(),
        "--config".into(),
        cfg("oxfmtrc.json"),
        shown.clone(),
      ],
    ),
    (
      "html-validate",
      bin.join("html-validate"),
      vec![
        "--config".into(),
        cfg("html-validate.json"),
        asset.join("index.html").display().to_string(),
      ],
    ),
  ];
  checks.extend(other_commands(&bin, &cfg, &shown, tests));
  checks
}

fn other_commands(
  bin: &Path,
  cfg: &impl Fn(&str) -> String,
  shown: &str,
  tests: &[String],
) -> Vec<(&'static str, PathBuf, Vec<String>)> {
  let mut node = vec!["--test".into()];
  node.extend(tests.iter().cloned());
  vec![
    (
      "ast-grep",
      PathBuf::from("ast-grep"),
      vec![
        "scan".into(),
        "--config".into(),
        cfg("sgconfig.yml"),
        "--report-style".into(),
        "short".into(),
        shown.into(),
      ],
    ),
    (
      "jscpd",
      bin.join("jscpd"),
      vec!["--config".into(), cfg("jscpd.json"), shown.into()],
    ),
    ("node --test", PathBuf::from("node"), node),
  ]
}

fn command(root: &Path, program: &Path, args: &[String]) -> Result<bool, String> {
  let status = Command::new(program)
    .args(args)
    .current_dir(root)
    .status()
    .map_err(|err| format!("web: cannot run {}: {err}", program.display()))?;
  Ok(status.success())
}

#[cfg(test)]
#[path = "tests/web_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/web_rules_test.rs"]
mod rules_tests;
