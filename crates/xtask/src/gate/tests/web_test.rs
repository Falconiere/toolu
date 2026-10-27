use std::path::PathBuf;

use super::{run, test_files};
use crate::Verdict;
use crate::options::Options;

#[test]
fn web_gate_clean_fixture_passes() {
  let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    .canonicalize()
    .unwrap();
  let options = Options {
    files: vec![PathBuf::from("fixtures/web-gate/clean")],
    only: vec!["web".to_owned()],
    root,
    ..Options::default()
  };
  assert_eq!(run(&options).unwrap(), Verdict::Clean);
}

#[test]
fn web_gate_clean_skips_absent_dashboard() {
  let root = tempfile::tempdir().unwrap();
  assert_eq!(
    run(&Options {
      root: root.path().to_path_buf(),
      ..Options::default()
    })
    .unwrap(),
    Verdict::Clean
  );
}

#[cfg(unix)]
#[test]
fn web_gate_clean_rejects_symlinked_asset_root() {
  let root = tempfile::tempdir().unwrap();
  let target = tempfile::tempdir().unwrap();
  let asset = root
    .path()
    .join("crates/epic-orchestrator/assets/dashboard");
  std::fs::create_dir_all(asset.parent().unwrap()).unwrap();
  std::os::unix::fs::symlink(target.path(), &asset).unwrap();
  let err = run(&Options {
    root: root.path().to_path_buf(),
    ..Options::default()
  })
  .unwrap_err();
  assert!(err.contains("symlink asset directory"), "{err}");
}

#[test]
fn web_gate_clean_requires_npm_ci_when_assets_exist() {
  let root = tempfile::tempdir().unwrap();
  let asset = root
    .path()
    .join("crates/epic-orchestrator/assets/dashboard");
  std::fs::create_dir_all(&asset).unwrap();
  let err = run(&Options {
    root: root.path().to_path_buf(),
    ..Options::default()
  })
  .unwrap_err();
  assert!(err.contains("npm ci"), "{err}");
}

#[test]
fn web_gate_clean_passes_explicit_node_test_files_in_order() {
  let asset = tempfile::tempdir().unwrap();
  assert!(
    test_files(asset.path())
      .unwrap_err()
      .contains("cannot read")
  );
  let tests = asset.path().join("tests");
  std::fs::create_dir(&tests).unwrap();
  std::fs::write(tests.join("readme.txt"), "not a test").unwrap();
  assert!(
    test_files(asset.path())
      .unwrap_err()
      .contains("no tests/*.test.js")
  );
  for name in ["b.test.js", "a.test.js"] {
    std::fs::write(tests.join(name), "").unwrap();
  }
  assert_eq!(
    test_files(asset.path()).unwrap(),
    ["a.test.js", "b.test.js"]
      .map(|name| tests.join(name).display().to_string())
      .to_vec()
  );
}

#[test]
fn web_gate_clean_rejects_missing_or_misplaced_fixture_path() {
  let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    .canonicalize()
    .unwrap();
  let options = Options {
    root,
    files: vec![PathBuf::from("fixtures/web-gate/missing")],
    only: vec!["web".to_owned()],
    ..Options::default()
  };
  assert!(
    run(&options)
      .unwrap_err()
      .contains("web asset directory missing")
  );
  let misplaced = Options {
    only: Vec::new(),
    ..options
  };
  assert!(
    run(&misplaced)
      .unwrap_err()
      .contains("requires `gate --only web PATH`")
  );
}
