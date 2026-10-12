use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_protocol::host::Host;
use toolu_runtime::config::quality::ThresholdSource;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::detect::project::NodePackageManager;

use super::{
  Limits, Pattern, Place, TsFile, compile, count_matches, has_extension, head, is_test_path,
  numbered, split_lines, with_excerpt, without_comment_lines,
};

static WORD: Pattern = Pattern::new(r"\bas\b");
static BROKEN: Pattern = Pattern::new("(");

#[test]
fn patterns_use_ascii_word_boundaries_and_dot_all() {
  assert!(WORD.is_match("x as Y"));
  assert!(!WORD.is_match("xas Y"));
  assert!(compile("a.b").is_some_and(|regex| regex.is_match("a\nb")));
  assert_eq!(WORD.remove_all("a as b"), "a  b");
  let group = Pattern::new("^export type ([A-Z]+)");
  assert_eq!(group.first_group("export type AB = 1"), Some("AB"));
  assert_eq!(group.first_group("const x = 1"), None);
}

#[test]
fn a_pattern_that_does_not_compile_matches_nothing() {
  assert!(!BROKEN.is_match("("));
  assert_eq!(BROKEN.first_group("("), None);
  assert_eq!(BROKEN.remove_all("(x"), "(x");
}

#[test]
fn lines_and_excerpts_follow_grep() {
  assert_eq!(split_lines("").len(), 0);
  assert_eq!(split_lines("a\nb\n"), ["a", "b"]);
  assert_eq!(split_lines("a\n\n"), ["a", ""]);
  let lines = split_lines("x as Y\n// a as b\nplain\nz as W\n");
  let rows = numbered(&lines, &WORD);
  assert_eq!(rows, ["1:x as Y", "2:// a as b", "4:z as W"]);
  assert_eq!(count_matches(&lines, &WORD), 3);
  assert_eq!(
    without_comment_lines(rows.clone()),
    ["1:x as Y", "4:z as W"]
  );
  assert_eq!(head(&rows, 2), "1:x as Y\n2:// a as b");
  assert_eq!(with_excerpt("H", "1:x"), Some("H\n1:x".to_owned()));
  assert_eq!(with_excerpt("H", ""), None);
}

#[test]
fn extensions_and_test_paths_are_case_sensitive() {
  assert!(has_extension("a/b.tsx", &["ts", "tsx"]));
  assert!(!has_extension("a/b.TS", &["ts"]));
  assert!(!has_extension("a/Makefile", &["ts"]));
  assert!(is_test_path("src/a.test.ts"));
  assert!(is_test_path("src/a.spec.tsx"));
  assert!(!is_test_path("src/a.test.js"));
}

#[test]
fn a_file_is_read_where_its_hook_runs() {
  let env = Env::default();
  let raw = serde_json::Map::new();
  let root = Path::new("/project");
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: None,
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let at = Place::of(&ctx);
  assert_eq!(at.cwd, root);
  let edited = EditedFile {
    path: "src/a.ts".to_owned(),
    absolute: root.join("src/a.ts"),
    removed: false,
  };
  let limits = Limits::default();
  assert_eq!(limits.file_source, ThresholdSource::Default);
  assert_eq!(
    (limits.file_lines, limits.fn_lines, limits.no_mocks),
    (300, 50, true)
  );
  let f = TsFile::new(&edited, "a\nb\n", at, limits, NodePackageManager::Npm);
  assert_eq!(f.path(), "src/a.ts");
  assert_eq!(f.lines, ["a", "b"]);
}
