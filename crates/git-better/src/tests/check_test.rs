//! The `check` verb's findings; the binary itself is exercised in
//! `crates/cli/tests/helpers/git_better_schema.rs`.

use super::findings;
use crate::report::Target;
use crate::settings::Settings;

#[test]
fn findings_cover_the_message_and_the_branch() {
  let settings = Settings::default();
  let found = findings(Some("fix stuff"), Some("my-branch"), &settings, false);
  let targets: Vec<Target> = found.iter().map(|finding| finding.target).collect();
  assert_eq!(targets, [Target::Message, Target::Branch]);
  assert_eq!(found[1].name, "my-branch");
  let clean = findings(
    Some("fix(x): y\n\nIntent: z"),
    Some("fix/y"),
    &settings,
    false,
  );
  assert_eq!(clean, []);
  assert_eq!(findings(Some("wip"), None, &settings, true).len(), 1);
}
