//! The epic-creator plugin's shipped bodies read by the real GitHub reader:
//! only `## Relationships` may create a dependency edge.

use super::text_blocker_refs;
use crate::epic_creator_rig::fence;

#[test]
fn epic_creator_example_sub_issue_has_exactly_its_relationships_edge() {
  let example = fence("## Example sub-issue").unwrap();
  assert_eq!(
    text_blocker_refs(example, "o/r", "o/r#40").unwrap(),
    ["o/r#12"]
  );
  assert_eq!(
    text_blocker_refs(example, "o/r", "o/r#12").unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn epic_creator_epic_and_sub_issue_templates_create_no_edges() {
  for heading in ["## Epic", "## Sub-issue"] {
    let template = fence(heading).unwrap();
    assert_eq!(
      text_blocker_refs(template, "o/r", "o/r#1").unwrap(),
      Vec::<String>::new(),
      "{heading}"
    );
  }
}

#[test]
fn epic_creator_a_prose_mention_outside_relationships_would_add_an_edge() {
  let example = fence("## Example sub-issue").unwrap();
  let padded = format!("{example}\nThis also depends on #7 landing first.");
  assert_eq!(
    text_blocker_refs(&padded, "o/r", "o/r#40").unwrap(),
    ["o/r#12", "o/r#7"]
  );
}
