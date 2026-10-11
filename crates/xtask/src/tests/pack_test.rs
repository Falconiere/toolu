use std::path::PathBuf;

use super::{Expectation, check_one, hook_source, run, shell_file};
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
fn native_shims_are_not_shell_files() {
  for shim in [
    "plugins/jev/scripts/jev.sh",
    "plugins/statusline/scripts/statusline.sh",
    "plugins/toolu-review/scripts/write-state.sh",
  ] {
    assert!(!shell_file(shim), "{shim}");
  }
  assert!(shell_file("plugins/x/scripts/run.sh"));
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
