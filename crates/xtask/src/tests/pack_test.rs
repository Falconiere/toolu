use std::path::PathBuf;

use super::{Expectation, check_one, hook_source, run};
use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn this_repository_packs() {
  let verdict = run(&Options {
    root: repo(),
    ..Options::default()
  })
  .expect("pack");
  assert_eq!(verdict, Verdict::Clean);
}

#[test]
fn a_hook_source_is_a_finding() {
  let expectation = Expectation {
    name: "pack-fixture",
    dir: ".",
    required: vec![
      "package.json".to_owned(),
      "plugins/x/hooks/dist/a.js".to_owned(),
      "plugins/x/hooks/dist/b.js".to_owned(),
    ],
    optional: vec![],
    forbidden: &[],
    patterns: &[hook_source],
    exact: false,
  };
  let files = vec![
    "package.json".to_owned(),
    "plugins/x/hooks/src/a.ts".to_owned(),
    "plugins/x/hooks/dist/a.js".to_owned(),
  ];
  assert_eq!(
    check_one(&expectation, &files),
    vec![
      "pack-fixture tarball is missing plugins/x/hooks/dist/b.js".to_owned(),
      "pack-fixture tarball must not contain plugins/x/hooks/src/a.ts".to_owned(),
    ]
  );
}

fn optional_expectation() -> Expectation {
  Expectation {
    name: "pack-fixture",
    dir: ".",
    required: vec!["package.json".to_owned()],
    optional: vec!["bin/linux-amd64/toolu".to_owned()],
    forbidden: &[],
    patterns: &[],
    exact: true,
  }
}

#[test]
fn an_absent_optional_file_is_clean() {
  assert_eq!(
    check_one(&optional_expectation(), &["package.json".to_owned()]),
    Vec::<String>::new()
  );
}

#[test]
fn a_present_optional_file_is_clean() {
  let files = vec![
    "package.json".to_owned(),
    "bin/linux-amd64/toolu".to_owned(),
  ];
  assert_eq!(
    check_one(&optional_expectation(), &files),
    Vec::<String>::new()
  );
}

#[test]
fn an_unlisted_file_is_a_finding_beside_optional_ones() {
  let files = vec![
    "package.json".to_owned(),
    "bin/linux-amd64/toolu".to_owned(),
    "bin/windows-amd64/toolu".to_owned(),
  ];
  assert_eq!(
    check_one(&optional_expectation(), &files),
    vec!["pack-fixture tarball has an undeclared file: bin/windows-amd64/toolu".to_owned()]
  );
}
