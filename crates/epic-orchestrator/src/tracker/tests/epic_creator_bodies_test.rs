//! The epic-creator plugin's shipped bodies read by the real GitHub reader:
//! only `## Relationships` may create a dependency edge.

use super::text_blocker_refs;

const BODIES: &str = include_str!(
  "../../../../../plugins/epic-creator/skills/epic-creator/references/issue-bodies.md"
);

/// The first `markdown` fence after `heading`.
fn fence<'a>(heading: &str) -> Result<&'a str, String> {
  BODIES
    .split_once(&format!("\n{heading}\n"))
    .and_then(|(_, rest)| rest.split_once("```markdown\n"))
    .and_then(|(_, rest)| rest.split_once("\n```"))
    .map(|(body, _)| body)
    .ok_or_else(|| format!("no markdown fence under {heading}"))
}

#[test]
fn epic_creator_example_sub_issue_has_exactly_its_relationships_edge() {
  let example = fence("## Example sub-issue").unwrap();
  assert_eq!(
    text_blocker_refs(example, "o/r", "o/r#40").unwrap(),
    ["o/r#12"]
  );
  assert!(
    text_blocker_refs(example, "o/r", "o/r#12")
      .unwrap()
      .is_empty()
  );
}

#[test]
fn epic_creator_epic_and_sub_issue_templates_create_no_edges() {
  for heading in ["## Epic", "## Sub-issue"] {
    let template = fence(heading).unwrap();
    assert!(
      text_blocker_refs(template, "o/r", "o/r#1")
        .unwrap()
        .is_empty(),
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
