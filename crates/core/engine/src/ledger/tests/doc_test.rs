//! Header fields, acceptance-criterion ids and `ac_refs` checks on real
//! files, with the values the TypeScript `ledger-parse.ts` gives.

use std::path::PathBuf;

use super::{AcRefs, check_ac_refs, doc_field, is_specless, parse_acs};

/// `body` as `name` in `dir`.
fn file(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
  let path = dir.path().join(name);
  std::fs::write(&path, body).unwrap();
  path
}

#[test]
fn doc_fields_are_cut_at_the_next_key_and_trimmed() {
  let dir = tempfile::tempdir().unwrap();
  let first = file(
    &dir,
    "a.md",
    "# P\n\n**Date:** 2031   **Status:** Approved   **Spec:** docs/spec.md\nnoise **Status:** Late\n",
  );
  assert_eq!(doc_field(&first, "Status"), "Approved");
  assert_eq!(doc_field(&first, "Spec"), "docs/spec.md");
  assert_eq!(doc_field(&first, "Date"), "2031");
  assert_eq!(doc_field(&first, "Missing"), "");
  assert_eq!(doc_field(&first, ""), "");
  let starred = file(&dir, "b.md", "**Status:**   Draft  **a*b:** x\n");
  assert_eq!(doc_field(&starred, "Status"), "Draft  **a*b:** x");
  let twice = file(&dir, "c.md", "x **Spec:** one **Spec:** two  **Next:** n\n");
  assert_eq!(doc_field(&twice, "Spec"), "two");
  let bare = file(&dir, "d.md", "**Status:** a **:** b **Ok:** c\n");
  assert_eq!(doc_field(&bare, "Status"), "a **:** b");
  let tabs = file(&dir, "e.md", "**Status:**\tv\t**K:**\r\n");
  assert_eq!(doc_field(&tabs, "Status"), "v");
  assert_eq!(doc_field(&dir.path().join("absent.md"), "Status"), "");
}

#[test]
fn acceptance_ids_come_from_their_section_once_each() {
  let dir = tempfile::tempdir().unwrap();
  let spec = file(
    &dir,
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
  let steps_doc = |json: &str| format!("## Steps (machine-readable)\n\n```json\n{json}\n```\n");
  file(&dir, "spec.md", "## Acceptance criteria\n- **AC-1:** a\n");
  let plan = "[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":[\"AC-9\",\"AC-1\",\"AC-10\",\"AC-9\"]},{\"id\":\"s2\",\"title\":\"t\",\"check\":\"c\",\"ac_refs\":\"AC-1\"}]";
  file(&dir, "plan.md", &steps_doc(plan));
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
  file(
    &dir,
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
fn none_and_empty_specs_are_specless() {
  assert!(is_specless("") && is_specless("NONE") && !is_specless("spec.md"));
}
