use std::path::PathBuf;

use super::{unreached, vendored};
use crate::workspace::{Member, Workspace};

#[test]
fn rust_files_outside_every_member_are_named() {
  let workspace = Workspace {
    root: PathBuf::from("/r"),
    members: vec![Member {
      name: "demo".to_owned(),
      dir: PathBuf::from("crates/demo"),
      is_lib: true,
    }],
    files: [
      "crates/demo/src/lib.rs",
      "crates/demo/fuzz/fuzz_targets/f.rs",
      "tools/x.rs",
      "tools/y.ts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect(),
  };
  assert_eq!(
    unreached(&workspace, &[]),
    ["reach tools/x.rs: outside every workspace member"]
  );
}

#[test]
fn a_vendored_crate_is_left_out_and_nothing_else() {
  let workspace = Workspace {
    root: PathBuf::from("/r"),
    members: Vec::new(),
    files: [
      "vendor/grammar/bindings/rust/lib.rs",
      "vendor/other/lib.rs",
      "tools/x.rs",
    ]
    .iter()
    .map(PathBuf::from)
    .collect(),
  };
  assert_eq!(
    unreached(&workspace, &[PathBuf::from("vendor/grammar")]),
    [
      "reach vendor/other/lib.rs: outside every workspace member",
      "reach tools/x.rs: outside every workspace member"
    ]
  );
}

#[test]
fn only_excluded_directories_under_vendor_count_as_vendored() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join("Cargo.toml"),
    "[workspace]\nmembers = []\nexclude = [\"vendor/grammar\", \"tools/escape\", \"vendor\"]\n",
  )
  .unwrap();
  assert_eq!(
    vendored(dir.path()).unwrap(),
    [PathBuf::from("vendor/grammar")]
  );
  std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
  assert_eq!(vendored(dir.path()).unwrap(), Vec::<PathBuf>::new());
  assert!(vendored(&dir.path().join("missing")).is_err());
}
