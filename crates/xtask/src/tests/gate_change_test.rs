use std::path::Path;
use std::process::Command;

use super::{run, verdict};
use crate::Verdict;
use crate::options::Options;

fn git(root: &Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
}

fn write(root: &Path, rel: &str, text: &str) {
  std::fs::create_dir_all(root.join(rel).parent().unwrap()).unwrap();
  std::fs::write(root.join(rel), text).unwrap();
}

fn repo(with_rules: bool) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  git(root, &["init", "-q", "-b", "main"]);
  write(root, "clippy.toml", "too-many-lines-threshold = 50\n");
  write(root, "crates/a/src/lib.rs", "//! a\n");
  if with_rules {
    let rules = std::fs::read_to_string(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../../tooling/conventions/guardrails/rust/rules.json"
    ))
    .unwrap();
    write(
      root,
      "tooling/conventions/guardrails/rust/rules.json",
      &rules,
    );
  }
  git(root, &["add", "-A"]);
  git(
    root,
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@example.com",
      "commit",
      "-qm",
      "base",
    ],
  );
  dir
}

fn options(root: &Path, title: Option<&str>) -> Options {
  Options {
    root: root.to_path_buf(),
    base: Some("HEAD".to_owned()),
    title: title.map(str::to_owned),
    ..Options::default()
  }
}

#[test]
fn gate_data_with_product_code_or_a_wrong_title_fails() {
  let gate = ["clippy.toml changed".to_owned()];
  let product = ["crates/a/src/lib.rs".to_owned()];
  let mixed = verdict(&gate, &product, None);
  assert_eq!(mixed.len(), 1);
  assert!(mixed[0].starts_with(
    "gate data changed (clippy.toml changed) together with product code (crates/a/src/lib.rs)"
  ));
  assert_eq!(verdict(&gate, &[], Some("feat: raise")).len(), 1);
  assert_eq!(
    verdict(&gate, &[], Some("chore(gates): raise the limit")),
    Vec::<String>::new()
  );
  assert_eq!(verdict(&gate, &[], None), Vec::<String>::new());
  assert_eq!(
    verdict(&[], &product, Some("feat: x")),
    Vec::<String>::new()
  );
}

#[test]
fn a_real_diff_is_classified_against_the_merge_base() {
  let dir = repo(true);
  let root = dir.path();
  assert_eq!(run(&options(root, None)).unwrap(), Verdict::Clean);
  write(root, "crates/a/src/lib.rs", "//! a changed\n");
  write(root, "crates/a/src/new.rs", "//! new\n");
  assert_eq!(
    run(&options(root, Some("feat: x"))).unwrap(),
    Verdict::Clean
  );
  write(root, "clippy.toml", "too-many-lines-threshold = 60\n");
  assert_eq!(run(&options(root, None)).unwrap(), Verdict::Findings);
}

#[test]
fn a_base_without_gate_data_is_the_bootstrap_and_a_bad_base_is_an_error() {
  let dir = repo(false);
  let root = dir.path();
  write(root, "clippy.toml", "too-many-lines-threshold = 60\n");
  assert_eq!(run(&options(root, None)).unwrap(), Verdict::Clean);
  let mut bad = options(root, None);
  bad.base = Some("no-such-ref".to_owned());
  assert!(
    run(&bad)
      .unwrap_err()
      .starts_with("git merge-base no-such-ref HEAD failed")
  );
}
