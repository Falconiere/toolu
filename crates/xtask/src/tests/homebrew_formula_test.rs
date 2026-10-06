use std::path::PathBuf;

use super::{digest, formula, run};
use crate::Verdict;
use crate::options::Options;

const DARWIN_ARM: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const DARWIN_AMD: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const LINUX_ARM: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const LINUX_AMD: &str = "4444444444444444444444444444444444444444444444444444444444444444";

/// A `SHA256SUMS` as `release_native.py` writes it, plus the extra files a
/// release carries.
fn sums() -> String {
  format!(
    "{DARWIN_AMD}  toolu-darwin-amd64.tar.gz\n{DARWIN_ARM}  toolu-darwin-arm64.tar.gz\n\
     {LINUX_AMD} *toolu-linux-amd64.tar.gz\n{LINUX_ARM}  toolu-linux-arm64.tar.gz\n\
     5555555555555555555555555555555555555555555555555555555555555555  toolu.spdx.json\n"
  )
}

fn options(dir: &tempfile::TempDir, tag: &str, text: &str) -> Options {
  let path = dir.path().join("SHA256SUMS");
  std::fs::write(&path, text).unwrap();
  Options {
    root: dir.path().to_path_buf(),
    files: vec![PathBuf::from(tag), path],
    ..Options::default()
  }
}

/// The `url` and `sha256` pair of one stanza, as they appear in the formula.
fn stanza(name: &str, hash: &str) -> String {
  format!(
    "url \"https://github.com/Falconiere/toolu/releases/download/v9.0.0/{name}\"\n      \
     sha256 \"{hash}\""
  )
}

#[test]
fn homebrew_formula_prints_the_four_checksums() {
  let text = formula("v9.0.0", &sums()).unwrap();
  assert!(text.starts_with("class Toolu < Formula\n"));
  assert!(text.contains("  version \"9.0.0\"\n"));
  assert!(text.contains("  license \"MIT\"\n"));
  for (name, hash) in [
    ("toolu-darwin-arm64.tar.gz", DARWIN_ARM),
    ("toolu-darwin-amd64.tar.gz", DARWIN_AMD),
    ("toolu-linux-arm64.tar.gz", LINUX_ARM),
    ("toolu-linux-amd64.tar.gz", LINUX_AMD),
  ] {
    assert!(text.contains(&stanza(name, hash)), "{name}");
  }
  let macos = text.find("on_macos do").unwrap();
  let linux = text.find("on_linux do").unwrap();
  assert!(macos < text.find("toolu-darwin-arm64").unwrap());
  assert!(text.find("toolu-darwin-amd64").unwrap() < linux);
  assert!(linux < text.find("toolu-linux-arm64").unwrap());
  assert!(text.contains("bin.install \"toolu\""));
  assert!(text.contains("assert_match \"toolu #{version}\""));
  assert!(!text.contains("pkgshare"));
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(run(&options(&dir, "v9.0.0", &sums())), Ok(Verdict::Clean));
}

#[test]
fn homebrew_formula_missing_checksum_exits_1() {
  let gap = sums().replace("toolu-linux-arm64.tar.gz", "toolu-linux-arm64.zip");
  assert_eq!(
    formula("v9.0.0", &gap),
    Err(vec![
      "SHA256SUMS has no sha256 for toolu-linux-arm64.tar.gz".to_owned()
    ])
  );
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(run(&options(&dir, "v9.0.0", &gap)), Ok(Verdict::Findings));
}

#[test]
fn homebrew_formula_rejects_bad_tags_files_and_arity() {
  let dir = tempfile::tempdir().unwrap();
  for tag in ["9.0.0", "v9.0", "v9.0.0-", "v9.0.0;rm"] {
    assert!(run(&options(&dir, tag, &sums())).is_err(), "{tag}");
  }
  let mut missing = options(&dir, "v9.0.0", &sums());
  missing.files[1] = dir.path().join("absent");
  assert!(run(&missing).unwrap_err().starts_with("cannot read "));
  let mut short = options(&dir, "v9.0.0", &sums());
  short.files.pop();
  assert!(run(&short).is_err());
}

#[test]
fn homebrew_formula_reads_only_well_formed_sums_lines() {
  assert_eq!(digest(&sums(), "toolu-linux-amd64.tar.gz"), Some(LINUX_AMD));
  assert_eq!(
    digest(
      "abc  toolu-linux-amd64.tar.gz\n",
      "toolu-linux-amd64.tar.gz"
    ),
    None
  );
  assert_eq!(digest(&sums(), "toolu-linux-amd64.tar"), None);
  assert_eq!(digest(&sums(), "toolu"), None);
}
