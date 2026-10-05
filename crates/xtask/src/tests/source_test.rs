use std::path::{Path, PathBuf};

use super::{Kind, Source};
use crate::workspace::Member;

fn demo() -> Member {
  Member {
    name: "demo".to_owned(),
    dir: PathBuf::from("crates/demo"),
    is_lib: true,
  }
}

fn kind(rel: &str) -> Kind {
  let member = demo();
  Source::new(Path::new(rel), Some(&member), "").kind
}

#[test]
fn files_are_classified_by_where_they_sit_in_their_crate() {
  assert_eq!(kind("crates/demo/src/lib.rs"), Kind::Src);
  assert_eq!(kind("crates/demo/src/tests/lib_test.rs"), Kind::UnitTest);
  assert_eq!(
    kind("crates/demo/src/gates/tests/gate_test.rs"),
    Kind::UnitTest
  );
  assert_eq!(kind("crates/demo/src/tests.rs"), Kind::Src);
  assert_eq!(
    kind("crates/demo/tests/black_box.rs"),
    Kind::IntegrationTest
  );
  assert_eq!(kind("crates/demo/fuzz/fuzz_targets/f.rs"), Kind::Other);
  assert_eq!(kind("crates/other/src/lib.rs"), Kind::Other);
  assert_eq!(Source::new(Path::new("x.rs"), None, "").kind, Kind::Other);
}

#[test]
fn a_non_utf8_file_is_a_parse_failure_not_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join("crates/demo/src")).unwrap();
  std::fs::write(dir.path().join("crates/demo/src/lib.rs"), [0xff_u8, 0xfe]).unwrap();
  let workspace = crate::workspace::Workspace {
    root: dir.path().to_path_buf(),
    members: vec![demo()],
    files: Vec::new(),
  };
  let source = Source::load(&workspace, Path::new("crates/demo/src/lib.rs")).unwrap();
  assert_eq!(
    source.ast.err(),
    Some((1, "the file is not UTF-8".to_owned()))
  );
  assert!(Source::load(&workspace, Path::new("missing.rs")).is_err());
}

#[test]
fn a_source_knows_its_src_path_and_parse_errors() {
  let member = demo();
  let source = Source::new(Path::new("crates/demo/src/a/b.rs"), Some(&member), "fn (");
  assert_eq!(source.in_src(), Some(PathBuf::from("a/b.rs")));
  assert!(!source.is_test());
  assert_eq!(source.display(), "crates/demo/src/a/b.rs");
  assert_eq!(source.ast.as_ref().err().map(|(line, _)| *line), Some(1));
  assert_eq!(Source::new(Path::new("x.rs"), None, "").in_src(), None);
  let test = Source::new(Path::new("crates/demo/tests/t.rs"), Some(&member), "");
  assert!(test.is_test());
}
