use std::path::PathBuf;

use super::{DEFAULT_BASE, Fixtures};
use crate::eval::{Class, Kind};
use crate::memory::{Memory, Status};
use crate::validate::fingerprint;

fn golden() -> Fixtures {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval");
  Fixtures::load(&root).unwrap()
}

#[test]
fn the_golden_set_covers_probe_defects_cross_file_prompt_fp_and_memory() {
  let fixtures = golden();
  let classes: Vec<Class> = fixtures.set().cases.iter().map(|case| case.class).collect();
  for wanted in [
    Class::Supported,
    Class::CrossFile,
    Class::PromptFalsePositive,
    Class::MemoryDrift,
    Class::MemoryUnreexamined,
    Class::MemoryMissing,
  ] {
    assert!(classes.contains(&wanted), "{wanted:?} missing");
  }
  for id in ["slice-underflow", "inverted-ownership"] {
    let case = fixtures.set().case(id).unwrap();
    assert_eq!(case.class, Class::Supported);
    assert!(case.expectations.iter().any(|row| row.kind == Kind::Defect));
  }
  let flagged = fixtures
    .set()
    .cases
    .iter()
    .flat_map(|case| &case.expectations)
    .filter(|row| row.kind == Kind::FalsePositive)
    .count();
  assert!(flagged >= 1);
}

#[test]
fn a_missing_directory_or_case_tree_is_an_error_not_an_empty_set() {
  let dir = tempfile::tempdir().unwrap();
  assert!(Fixtures::load(dir.path()).is_err());
  std::fs::write(
    dir.path().join("expectations.json"),
    r#"{"version":1,"cases":[{"id":"ghost","class":"supported","expectations":[]}]}"#,
  )
  .unwrap();
  let error = Fixtures::load(dir.path()).unwrap_err();
  assert!(error.contains("ghost"), "{error}");
}

#[test]
fn materialize_builds_a_real_two_commit_repository() {
  let fixtures = golden();
  let case = fixtures.set().case("slice-underflow").unwrap();
  let repo = fixtures.materialize_repo(case).unwrap();
  assert!(!repo.path().join("files").exists());
  let tip = std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap();
  assert!(tip.contains("items.len() - count"));
  let log = std::process::Command::new("git")
    .current_dir(repo.path())
    .args(["rev-list", "--count", "HEAD"])
    .output()
    .unwrap();
  assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), "2");
  let base = std::process::Command::new("git")
    .current_dir(repo.path())
    .args(["show", &format!("{}:src/lib.rs", fixtures.base_rev(case))])
    .output()
    .unwrap();
  assert!(String::from_utf8_lossy(&base.stdout).contains("saturating_sub"));
}

#[test]
fn a_tip_tree_replaces_files_the_base_had() {
  let fixtures = golden();
  let case = fixtures.set().case("cross-file-defect").unwrap();
  let repo = fixtures.materialize_repo(case).unwrap();
  let pages = std::fs::read_to_string(repo.path().join("src/pages.rs")).unwrap();
  assert!(!pages.contains("div_ceil"));
  assert_eq!(fixtures.base_rev(case), DEFAULT_BASE);
}

#[test]
fn memory_seeds_load_with_line_independent_fingerprints() {
  let fixtures = golden();
  for id in ["memory-drift", "memory-unreexamined"] {
    let case = fixtures.set().case(id).unwrap();
    let memory = fixtures.memory(case).unwrap().unwrap();
    let prior = memory.prior.first().unwrap();
    assert_eq!(prior.status, Status::Open);
    assert_eq!(
      prior.fingerprint,
      fingerprint(&prior.path, &prior.category, &prior.claim)
    );
  }
  let missing = fixtures.set().case("memory-missing").unwrap();
  assert_eq!(
    fixtures.memory(missing).unwrap().unwrap(),
    Memory::default()
  );
  let none = fixtures.set().case("slice-underflow").unwrap();
  assert!(fixtures.memory(none).unwrap().is_none());
}
