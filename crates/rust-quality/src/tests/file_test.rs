use std::path::Path;
use std::sync::LazyLock;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

use super::{Limits, RsFile, Rx, hit, rx, split_lines, test_file_name};

static WORD: Rx = LazyLock::new(|| rx(r"\bfn\b"));
static SPACE: Rx = LazyLock::new(|| rx(r"^a[[:space:]]b.c$"));
static BROKEN: Rx = LazyLock::new(|| rx("("));

#[test]
fn patterns_read_posix_classes_and_ascii_word_boundaries() {
  assert!(hit(&WORD, "pub fn x()"));
  assert!(
    hit(&WORD, "éfn"),
    "an accented letter is no ASCII word character"
  );
  assert!(!hit(&WORD, "fnord"));
  assert!(hit(&SPACE, "a\tb\nc") && hit(&SPACE, "a\rb\rc"));
  assert!(BROKEN.is_none() && !hit(&BROKEN, "("));
}

#[test]
fn lines_are_awk_records_without_a_final_empty_one() {
  assert_eq!(split_lines(""), [""; 0]);
  assert_eq!(split_lines("a\nb\n"), ["a", "b"]);
  assert_eq!(split_lines("a\n\nb"), ["a", "", "b"]);
  assert_eq!(split_lines("\n"), [""]);
  assert_eq!(split_lines("a\r\n"), ["a\r"]);
}

#[test]
fn test_file_names_are_read_from_the_basename() {
  assert!(test_file_name("crates/x/src/tests/lib_test.rs"));
  assert!(test_file_name("a_tests.rs"));
  assert!(!test_file_name("crates/x_test.rs/lib.rs"));
  assert!(!test_file_name("src/latest.rs"));
}

#[test]
fn path_rules_test_the_scope_and_messages_keep_the_path() {
  let file = EditedFile {
    path: "src/lib.rs".to_owned(),
    absolute: Path::new("/r/src/lib.rs").to_path_buf(),
    removed: false,
  };
  let limits = Limits::default();
  let env = Env::default();
  let f = RsFile {
    file: &file,
    scope: "/src/lib.rs",
    lines: Vec::new(),
    limits: &limits,
    env: &env,
    cwd: Path::new("/r"),
  };
  assert!(f.path_has("/src/"));
  assert_eq!(f.path(), "src/lib.rs");
  assert_eq!(
    (limits.file_lines, limits.fn_lines, limits.impl_lines),
    (500, 50, 200)
  );
  assert!(limits.no_mocks && limits.unsafe_exemptions.is_empty());
}
