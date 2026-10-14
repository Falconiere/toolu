use std::path::{Path, PathBuf};

use super::{check, run};
use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A temp root with this repository's workflows, release config and installer.
fn copy() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let workflows = dir.path().join(".github/workflows");
  std::fs::create_dir_all(&workflows).unwrap();
  for entry in std::fs::read_dir(repo().join(".github/workflows")).unwrap() {
    let path = entry.unwrap().path();
    std::fs::copy(&path, workflows.join(path.file_name().unwrap())).unwrap();
  }
  for file in [
    ".github/code-review-prompt.md",
    "release-please-config.json",
    "install.sh",
  ] {
    std::fs::copy(repo().join(file), dir.path().join(file)).unwrap();
  }
  dir
}

/// Replace `from` with `to` in a copied workflow; `from` must be there.
fn mutate(root: &Path, name: &str, from: &str, to: &str) {
  let path = root.join(".github/workflows").join(name);
  let text = std::fs::read_to_string(&path).unwrap();
  assert!(text.contains(from), "{name} lacks {from:?}");
  std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn committed_workflows_pass() {
  assert_eq!(check(&repo()).unwrap(), Vec::<String>::new());
  let options = Options::parse(&[]).unwrap();
  assert_eq!(run(&options).unwrap(), Verdict::Clean);
}

#[test]
fn absent_workflows_fail_closed() {
  let temp = tempfile::TempDir::new().unwrap();
  let options = Options::parse(&[
    "--root".to_owned(),
    temp.path().to_string_lossy().to_string(),
  ])
  .unwrap();
  let err = run(&options).unwrap_err();
  assert!(
    err.starts_with("release-please-config.json: cannot read or parse"),
    "{err}"
  );
  std::fs::write(temp.path().join("release-please-config.json"), "{}").unwrap();
  assert_eq!(run(&options).unwrap(), Verdict::Findings);
  let found = check(temp.path()).unwrap();
  assert!(
    found
      .iter()
      .any(|line| line.starts_with("tests.yml: cannot read or parse"))
  );
}

#[test]
fn a_copy_passes_and_each_mutation_names_its_break() {
  let dir = copy();
  assert_eq!(check(dir.path()).unwrap(), Vec::<String>::new());
  mutate(
    dir.path(),
    "tests.yml",
    "cargo xtask ci-changes",
    "cargo xtask ci-change",
  );
  mutate(
    dir.path(),
    "release-native.yml",
    "--locked --bin toolu",
    "--bin toolu",
  );
  mutate(
    dir.path(),
    "npm-publish.yml",
    "cancel-in-progress: false",
    "cancel-in-progress: true",
  );
  assert_eq!(
    check(dir.path()).unwrap(),
    [
      "tests.yml:changes must run cargo xtask ci-changes",
      "release-native.yml build must use --locked",
      "npm-publish.yml must not cancel an in-flight publication",
    ]
  );
}

#[test]
fn invalid_yaml_is_a_finding_not_a_pass() {
  let dir = copy();
  let path = dir.path().join(".github/workflows/advisory-audit.yml");
  std::fs::write(&path, "jobs: [unclosed\n").unwrap();
  let found = check(dir.path()).unwrap();
  assert!(
    found
      .iter()
      .any(|line| line.starts_with("advisory-audit.yml: cannot read or parse")),
    "{found:?}"
  );
}
