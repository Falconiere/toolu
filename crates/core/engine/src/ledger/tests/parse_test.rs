//! Plan and spec parsing on real files, with the values and messages the
//! TypeScript `ledger-parse.ts` gives for the same documents.

use std::path::{Path, PathBuf};

use toolu_runtime::json::jq_text;

use super::{
  AcRefs, ParseFailure, check_ac_refs, doc_field, is_file, is_specless, lines, parse_acs,
  parse_steps, resolve,
};

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
fn doc_fields_are_cut_at_the_next_key_and_trimmed() {
  let dir = tempfile::tempdir().unwrap();
  let first = write(
    dir.path(),
    "a.md",
    "# P\n\n**Date:** 2031   **Status:** Approved   **Spec:** docs/spec.md\nnoise **Status:** Late\n",
  );
  assert_eq!(doc_field(&first, "Status"), "Approved");
  assert_eq!(doc_field(&first, "Spec"), "docs/spec.md");
  assert_eq!(doc_field(&first, "Date"), "2031");
  assert_eq!(doc_field(&first, "Missing"), "");
  assert_eq!(doc_field(&first, ""), "");
  let starred = write(dir.path(), "b.md", "**Status:**   Draft  **a*b:** x\n");
  assert_eq!(doc_field(&starred, "Status"), "Draft  **a*b:** x");
  let twice = write(
    dir.path(),
    "c.md",
    "x **Spec:** one **Spec:** two  **Next:** n\n",
  );
  assert_eq!(doc_field(&twice, "Spec"), "two");
  let bare = write(dir.path(), "d.md", "**Status:** a **:** b **Ok:** c\n");
  assert_eq!(doc_field(&bare, "Status"), "a **:** b");
  let tabs = write(dir.path(), "e.md", "**Status:**\tv\t**K:**\r\n");
  assert_eq!(doc_field(&tabs, "Status"), "v");
  assert_eq!(doc_field(&dir.path().join("absent.md"), "Status"), "");
}

#[test]
fn acceptance_ids_come_from_their_section_once_each() {
  let dir = tempfile::tempdir().unwrap();
  let spec = write(
    dir.path(),
    "s.md",
    "# S\n## Acceptance criteria  \r\n- **AC-1:** a **AC-2:** b\n- **AC-x:** **AC-3:** c\n- **AC-1:** dup\n## Other\n- **AC-9:** out\n## Acceptance criteria\n- **AC-4:**\n",
  );
  assert_eq!(parse_acs(&spec), ["AC-1", "AC-3", "AC-4"]);
  assert_eq!(
    parse_acs(&dir.path().join("absent.md")),
    Vec::<String>::new()
  );
}

#[test]
fn ac_refs_are_checked_against_the_declared_spec() {
  let dir = tempfile::tempdir().unwrap();
  write(
    dir.path(),
    "spec.md",
    "## Acceptance criteria\n- **AC-1:** a\n",
  );
  let plan = "[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":[\"AC-9\",\"AC-1\",\"AC-10\",\"AC-9\"]},{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":\"AC-1\"}]";
  write(dir.path(), "plan.md", &steps_doc(plan));
  let dangling = |ids: &[&str]| AcRefs {
    ok: ids.is_empty(),
    dangling: ids.iter().map(|id| (*id).to_owned()).collect(),
    message: None,
  };
  assert_eq!(
    check_ac_refs("plan.md", "spec.md", dir.path()),
    dangling(&["AC-10", "AC-9"])
  );
  assert_eq!(check_ac_refs("plan.md", "None", dir.path()), dangling(&[]));
  write(
    dir.path(),
    "covered.md",
    &steps_doc("[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":[\"AC-1\"]}]"),
  );
  assert_eq!(
    check_ac_refs("covered.md", "spec.md", dir.path()),
    dangling(&[])
  );
  let broken = check_ac_refs("absent.md", "spec.md", dir.path());
  assert!(!broken.ok);
  assert_eq!(
    broken.message.as_deref(),
    Some("plan-ledger-parse: plan doc not found: absent.md")
  );
}

#[test]
fn helpers_follow_node_and_bash() {
  assert!(is_specless("") && is_specless("NONE") && !is_specless("spec.md"));
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
