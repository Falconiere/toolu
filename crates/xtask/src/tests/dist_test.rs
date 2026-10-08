use std::path::{Path, PathBuf};
use std::process::Command;

use super::{build_one, checksums, drift, drift_with};
use crate::Verdict;
use crate::ci_yaml::bun_binary;
use crate::homebrew_formula::digest;
use crate::options::Options;

fn options(root: PathBuf, files: &[&str]) -> Options {
  Options {
    root,
    files: files.iter().map(PathBuf::from).collect(),
    ..Options::default()
  }
}

fn write_source(root: &Path) {
  let src = root.join("plugins/demo/hooks/src/pre-tools.ts");
  std::fs::create_dir_all(src.parent().unwrap()).unwrap();
  std::fs::write(&src, "console.log(\"one\");\n").unwrap();
}

#[test]
fn a_matching_bundle_passes() {
  let root = tempfile::tempdir().unwrap();
  write_source(root.path());
  let outfile = root.path().join("plugins/demo/hooks/dist/pre-tools.js");
  std::fs::create_dir_all(outfile.parent().unwrap()).unwrap();
  build_one(
    &bun_binary().unwrap(),
    root.path(),
    "plugins/demo/hooks/src/pre-tools.ts",
    &outfile,
  )
  .unwrap();
  assert_eq!(drift(root.path()).unwrap(), Vec::<String>::new());
  assert_eq!(
    super::run(&options(root.path().to_path_buf(), &["check"])).unwrap(),
    Verdict::Clean
  );
}

#[test]
fn a_stale_bundle_names_the_path() {
  let root = tempfile::tempdir().unwrap();
  write_source(root.path());
  let outfile = root.path().join("plugins/demo/hooks/dist/pre-tools.js");
  std::fs::create_dir_all(outfile.parent().unwrap()).unwrap();
  std::fs::write(&outfile, "stale\n").unwrap();
  let problems = drift(root.path()).unwrap();
  assert!(
    problems
      .iter()
      .any(|line| line.contains("plugins/demo/hooks/dist/pre-tools.js")),
    "{problems:?}"
  );
  assert_eq!(
    super::run(&options(root.path().to_path_buf(), &["check"])).unwrap(),
    Verdict::Findings
  );
}

#[test]
fn a_missing_bun_is_an_error() {
  let root = tempfile::tempdir().unwrap();
  write_source(root.path());
  let err = drift_with(root.path(), Path::new("/no/such/bun")).unwrap_err();
  assert!(err.contains("bun is not installed"), "{err}");
}

fn gzip(dir: &Path, name: &str) {
  let raw = dir.join(format!("{name}.bin"));
  std::fs::write(&raw, b"hi").unwrap();
  let output = Command::new("gzip")
    .args(["-n", "-c"])
    .arg(&raw)
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  std::fs::write(dir.join(name), output.stdout).unwrap();
}

#[test]
fn a_missing_archive_names_each_gap_and_writes_nothing() {
  let dir = tempfile::tempdir().unwrap();
  gzip(dir.path(), "toolu-linux-amd64.tar.gz");
  let problems = checksums(dir.path()).unwrap();
  for name in [
    "toolu-darwin-arm64.tar.gz",
    "toolu-darwin-amd64.tar.gz",
    "toolu-linux-arm64.tar.gz",
  ] {
    assert!(
      problems.iter().any(|line| line.contains(name)),
      "{problems:?}"
    );
  }
  assert!(!dir.path().join("SHA256SUMS").exists());
  assert_eq!(
    super::run(&options(
      dir.path().to_path_buf(),
      &["checksums", &dir.path().to_string_lossy()]
    ))
    .unwrap(),
    Verdict::Findings
  );
}

#[test]
fn four_archives_write_sums_homebrew_can_read() {
  let dir = tempfile::tempdir().unwrap();
  for name in super::ARCHIVES {
    gzip(dir.path(), name);
  }
  assert_eq!(checksums(dir.path()).unwrap(), Vec::<String>::new());
  let text = std::fs::read_to_string(dir.path().join("SHA256SUMS")).unwrap();
  for name in super::ARCHIVES {
    assert!(digest(&text, name).is_some(), "{name} in {text}");
  }
}
