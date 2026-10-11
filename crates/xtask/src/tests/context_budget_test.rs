use std::path::{Path, PathBuf};

use super::{DOC_BUDGETS, Mode, check, extract_description, render, run, word_count};
use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options(root: PathBuf, mode: &str) -> Options {
  Options {
    root,
    files: if mode.is_empty() {
      Vec::new()
    } else {
      vec![PathBuf::from(mode)]
    },
    ..Options::default()
  }
}

fn write(dir: &Path, rel: &str, text: &str) {
  let path = dir.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

#[test]
fn an_under_budget_doc_passes() {
  let dir = tempfile::tempdir().unwrap();
  write(
    dir.path(),
    "plugins/toolu/hooks/docs/post-compaction.md",
    "one two\n",
  );
  let lines = check(dir.path(), Mode::Docs);
  assert_eq!(lines.len(), 7);
  assert_eq!(
    render(&lines[3]).as_str(),
    "ok   post-compaction 2w (<= 28)"
  );
}

#[test]
fn the_real_post_compaction_doc_is_within_28_words() {
  let lines = check(&repo(), Mode::Docs);
  let line = lines
    .iter()
    .find(|line| line.text.starts_with("post-compaction "))
    .unwrap();
  assert!(!line.red, "{}", line.text);
  assert!(line.text.contains("<= 28"), "{}", line.text);
  assert_eq!(run(&options(repo(), "docs")).unwrap(), Verdict::Clean);
}

#[test]
fn an_over_budget_skill_exits_1() {
  let dir = tempfile::tempdir().unwrap();
  let words = (0..51)
    .map(|index| format!("word{index}"))
    .collect::<Vec<_>>()
    .join(" ");
  let text = format!("---\nname: brainstorm\ndescription: brainstorm trade-offs {words}\n---\n");
  write(
    dir.path(),
    "plugins/brainstorm/skills/brainstorm/SKILL.md",
    &text,
  );
  let lines = check(dir.path(), Mode::Skills);
  let hit = lines
    .iter()
    .find(|line| line.text.contains("brainstorm desc "))
    .unwrap();
  assert!(hit.red, "{}", hit.text);
  assert!(hit.text.contains("EXCEEDS 50"), "{}", hit.text);
  assert_eq!(
    run(&options(dir.path().to_path_buf(), "skills")).unwrap(),
    Verdict::Findings
  );
}

#[test]
fn a_missing_doc_is_red_and_an_unknown_mode_is_exit_2() {
  let dir = tempfile::tempdir().unwrap();
  let lines = check(dir.path(), Mode::Docs);
  assert_eq!(
    render(&lines[0]),
    "RED  session-start: MISSING plugins/toolu/hooks/docs/session-start.md"
  );
  let err = run(&options(dir.path().to_path_buf(), "bogus")).unwrap_err();
  assert_eq!(err, "usage: cargo xtask context-budget [docs|skills]");
}

#[test]
fn folded_descriptions_unicode_spaces_and_missing_phrases() {
  let dir = tempfile::tempdir().unwrap();
  write(
    dir.path(),
    "folded.md",
    "---\nname: x\ndescription: >\n  alpha beta\n  gamma delta\n---\n",
  );
  assert_eq!(
    extract_description(&dir.path().join("folded.md")).as_deref(),
    Some("alpha beta gamma delta")
  );
  write(dir.path(), "u.md", "a\u{00a0}b\u{3000}c d\n");
  assert_eq!(
    word_count(&std::fs::read_to_string(dir.path().join("u.md")).unwrap()),
    4
  );
  write(
    dir.path(),
    "plugins/brainstorm/skills/brainstorm/SKILL.md",
    "---\nname: s\ndescription: short blurb without the marker\n---\n",
  );
  let lines = check(dir.path(), Mode::Skills);
  assert!(
    lines
      .iter()
      .any(|line| { line.red && line.text.contains("missing trigger phrase \"brainstorm\"") })
  );
}

#[test]
fn doc_names_match_the_session_hook() {
  let source =
    std::fs::read_to_string(repo().join("plugins/toolu/hooks/src/lifecycle/session-docs.ts"))
      .unwrap();
  let pattern = regex::Regex::new(r#""([a-z-]+)\.md""#).unwrap();
  let mut rendered: Vec<&str> = pattern
    .captures_iter(&source)
    .filter_map(|caps| caps.get(1).map(|name| name.as_str()))
    .collect();
  rendered.sort_unstable();
  rendered.dedup();
  let mut expected: Vec<&str> = DOC_BUDGETS.iter().map(|(name, _)| *name).collect();
  expected.sort_unstable();
  assert_eq!(rendered, expected);
}
