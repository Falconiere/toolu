//! The epic-creator plugin's shipped example sub-issue routes as Standard.

use super::{Evidence, heuristic_tier};

const BODIES: &str = include_str!(
  "../../../../../plugins/epic-creator/skills/epic-creator/references/issue-bodies.md"
);

/// The example's title and body.
fn example() -> Result<(&'static str, &'static str), String> {
  let (_, rest) = BODIES
    .split_once("\n## Example sub-issue\n")
    .ok_or("no example section")?;
  let (_, title) = rest.split_once("Title: `").ok_or("no example title")?;
  let (title, rest) = title.split_once('`').ok_or("unclosed example title")?;
  let (_, body) = rest.split_once("```markdown\n").ok_or("no example fence")?;
  let (body, _) = body.split_once("\n```").ok_or("unclosed example fence")?;
  Ok((title, body))
}

#[test]
fn epic_creator_example_sub_issue_stays_under_the_routing_thresholds() {
  let (_, body) = example().unwrap();
  assert!(
    body.chars().count() < 1200,
    "{} chars",
    body.chars().count()
  );
  assert!(body.matches("- [ ]").count() < 6);
}

#[test]
fn epic_creator_example_sub_issue_routes_as_standard() {
  let (title, body) = example().unwrap();
  let evidence = Evidence {
    key: "o-r-40".into(),
    title: title.into(),
    excerpt: body.trim().into(),
    ..Evidence::default()
  };
  assert_eq!(heuristic_tier(&evidence), 1);
}
