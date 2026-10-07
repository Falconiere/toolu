//! Plan and spec parsing on real files, with the values and messages the
//! TypeScript `ledger-parse.ts` gives for the same documents.

use std::path::{Path, PathBuf};

use toolu_runtime::json::jq_text;

use super::{ParseFailure, is_file, lines, parse_steps, resolve};

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
  let path = dir.join(name);
  std::fs::write(&path, body).unwrap();
  path
}

fn steps_doc(json: &str) -> String {
  format!("# Plan\n\n## Steps (machine-readable)\n\n```json\n{json}\n```\n")
}

fn failure(exit: u8, message: &str) -> ParseFailure {
  ParseFailure {
    exit,
    message: format!("plan-ledger-parse: {message}"),
  }
}

#[test]
fn steps_are_normalized_with_existing_keys_in_place() {
  let dir = tempfile::tempdir().unwrap();
  let body = "## Steps (machine-readable)\r\n\n```json \n[{\"model\":null,\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":false,\"zz\":1},\
{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"paths\":[\"a\"],\"input\":{\"x\":1}}]\n```\n```json\n[1]\n```\n";
  write(dir.path(), "ok.md", body);
  let steps = parse_steps("ok.md", dir.path()).unwrap();
  let texts: Vec<String> = steps.iter().map(|step| jq_text(step, false)).collect();
  assert_eq!(
    texts,
    [
      "{\"model\":null,\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":[],\"zz\":1,\"depends_on\":[],\"paths\":[],\"input\":null}",
      "{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"paths\":[\"a\"],\"input\":{\"x\":1},\"ac_refs\":[],\"depends_on\":[],\"model\":null}",
    ]
  );
}

#[test]
fn a_missing_doc_block_or_field_fails_with_exit_1() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(
    parse_steps("missing.md", dir.path()),
    Err(failure(1, "plan doc not found: missing.md"))
  );
  write(
    dir.path(),
    "none.md",
    "```json\n[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\"}]\n```\n## Steps (machine-readable)\n",
  );
  assert_eq!(
    parse_steps("none.md", dir.path()),
    Err(failure(
      1,
      "no '## Steps (machine-readable)' json block in none.md"
    ))
  );
  for (name, json) in [
    ("empty.md", "[]"),
    ("object.md", "{\"id\":\"s1\"}"),
    (
      "blank.md",
      "[{\"id\":\"s1\",\"title\":\"\",\"check\":\"c\"}]",
    ),
    ("number.md", "[{\"id\":3,\"title\":\"t\",\"check\":\"c\"}]"),
    ("broken.md", "[{\"id\":"),
  ] {
    write(dir.path(), name, &steps_doc(json));
    let message =
      format!("steps block in {name} is not a non-empty array of {{id,title,check}} strings");
    assert_eq!(
      parse_steps(name, dir.path()),
      Err(failure(1, &message)),
      "{name}"
    );
  }
}

#[test]
fn an_unroutable_model_fails_with_exit_1_and_bad_paths_with_exit_2() {
  let dir = tempfile::tempdir().unwrap();
  let models = "[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"model\":false},{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"model\":\"gpt\"}]";
  write(dir.path(), "model.md", &steps_doc(models));
  let allowed = "allowed: haiku sonnet opus fable inherit";
  assert_eq!(
    parse_steps("model.md", dir.path()),
    Err(failure(
      1,
      &format!("invalid step model in model.md (s1=false, s2=gpt); {allowed}")
    ))
  );
  let paths = "[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"paths\":\"a\"},{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"paths\":[\"\",1]},{\"id\":\"s3\",\"title\":\"t\",\"check\":\"c\",\"paths\":null}]";
  write(dir.path(), "paths.md", &steps_doc(paths));
  let message = "invalid step paths in paths.md (s1, s2); expected an array of non-empty strings";
  assert_eq!(
    parse_steps("paths.md", dir.path()),
    Err(failure(2, message))
  );
}

#[test]
fn helpers_follow_node_and_bash() {
  assert_eq!(
    resolve(Path::new("/a/b"), "../c/./d"),
    PathBuf::from("/a/c/d")
  );
  assert_eq!(resolve(Path::new("/a"), "/x/../y"), PathBuf::from("/y"));
  let dir = tempfile::tempdir().unwrap();
  let file = write(dir.path(), "f.md", "one\r\ntwo\n\nthree");
  assert_eq!(lines(&file), ["one\r", "two", "", "three"]);
  assert!(is_file(&file) && !is_file(dir.path()));
  assert_eq!(lines(&dir.path().join("absent")), Vec::<String>::new());
}
