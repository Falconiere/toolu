//! The epic-creator plugin's shipped issue bodies, for the reader and router
//! tests that must agree with them.

const BODIES: &str =
  include_str!("../../../../plugins/epic-creator/skills/epic-creator/references/issue-bodies.md");

/// The text after the `heading` line of the shipped bodies.
fn after(heading: &str) -> Result<&'static str, String> {
  BODIES
    .split_once(&format!("\n{heading}\n"))
    .map(|(_, rest)| rest)
    .ok_or_else(|| format!("no section {heading}"))
}

/// The first `markdown` fence after `heading`.
pub(crate) fn fence(heading: &str) -> Result<&'static str, String> {
  after(heading)?
    .split_once("```markdown\n")
    .and_then(|(_, rest)| rest.split_once("\n```"))
    .map(|(body, _)| body)
    .ok_or_else(|| format!("no markdown fence under {heading}"))
}

/// The example sub-issue's title, from its `Title:` line.
pub(crate) fn example_title() -> Result<&'static str, String> {
  after("## Example sub-issue")?
    .split_once("Title: `")
    .and_then(|(_, rest)| rest.split_once('`'))
    .map(|(title, _)| title)
    .ok_or_else(|| "no example title".to_owned())
}
