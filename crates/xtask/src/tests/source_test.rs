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
