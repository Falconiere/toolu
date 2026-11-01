//! The epic-creator skill's contract: small, it confirms once before any
//! write, it writes through GitHub's native sub-issue and dependency
//! endpoints, and it proves the result with the engine's own reader.

use std::path::Path;

#[path = "helpers/skill_text.rs"]
mod skill_text;

use skill_text::{body, section};

/// A fallible helper result; the error says what was missing.
type Res<T> = Result<T, String>;

/// One shipped epic-creator file.
fn shipped(rel: &str) -> Res<String> {
  let root =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/epic-creator/skills/epic-creator");
  std::fs::read_to_string(root.join(rel)).map_err(|err| format!("{rel}: {err}"))
}

#[test]
fn the_skill_body_is_under_150_lines() {
  let text = shipped("SKILL.md").unwrap();
  let lines = body(&text).unwrap().lines().count();
  assert!(lines < 150, "the skill body has {lines} lines");
}

#[test]
fn the_skill_confirms_once_before_it_creates_anything() {
  let text = shipped("SKILL.md").unwrap();
  let (before, after) = text.split_once("\n## 5. Create").unwrap();
  let section = section(before, "4. Confirm").unwrap();
  assert!(
    section.contains("--dry-run") && section.contains("nothing written"),
    "{section}"
  );
  assert!(after.contains("references/github-writes.md"));
}

#[test]
fn writes_use_the_native_link_and_dependency_endpoints_and_never_repeat_a_create() {
  let writes = shipped("references/github-writes.md").unwrap();
  for endpoint in [
    "/sub_issues\" -F sub_issue_id=",
    "/dependencies/blocked_by\" -F issue_id=",
  ] {
    assert!(writes.contains(endpoint), "missing {endpoint}");
  }
  assert!(writes.contains("Do not repeat a create"));
}

#[test]
fn the_skill_proves_the_epic_with_the_engine_reader_and_router() {
  let text = shipped("SKILL.md").unwrap();
  let prove = section(&text, "6. Prove").unwrap();
  assert!(prove.contains("toolu --json epic graph"), "{prove}");
  assert!(prove.contains("toolu epic route --graph"), "{prove}");
  assert!(prove.contains("--dry-run"), "{prove}");
}
