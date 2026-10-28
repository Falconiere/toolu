//! Semantic branch names: the parts, every check and the corrected example.

use super::{Branch, check_branch, example_branch, parse};
use crate::settings::Settings;

fn codes(name: &str, settings: &Settings) -> Vec<&'static str> {
  check_branch(name, settings)
    .into_iter()
    .map(|violation| violation.code)
    .collect()
}

#[test]
fn a_branch_splits_into_type_reference_and_slug() {
  assert_eq!(
    parse("feat/653-pause-and-resume-an-agent"),
    Some(Branch {
      kind: "feat",
      reference: Some("653"),
      slug: "pause-and-resume-an-agent",
    })
  );
  assert_eq!(
    parse("fix/ENG-12-retry-timeouts"),
    Some(Branch {
      kind: "fix",
      reference: Some("ENG-12"),
      slug: "retry-timeouts",
    })
  );
  assert_eq!(
    parse("docs/readme").map(|branch| branch.reference),
    Some(None)
  );
  assert_eq!(parse("main"), None);
}

#[test]
fn semantic_branches_pass() {
  let settings = Settings::default();
  for name in [
    "feat/653-pause-and-resume-an-agent",
    "fix/ENG-12-retry-timeouts",
    "docs/readme",
    "chore/bump-2",
  ] {
    assert_eq!(codes(name, &settings), Vec::<&str>::new(), "{name}");
  }
}

#[test]
fn each_check_names_what_is_wrong() {
  let settings = Settings::default();
  let cases: [(&str, &[&str]); 6] = [
    ("my-branch", &["branch-format"]),
    ("feature/x", &["branch-type"]),
    ("feat/a/b", &["branch-depth"]),
    ("feat/Add_Thing", &["branch-slug"]),
    ("feat/653", &["branch-slug"]),
    ("feat/x--y", &["branch-slug"]),
  ];
  for (name, expected) in cases {
    assert_eq!(codes(name, &settings), expected, "{name}");
  }
  let long = format!("feat/{}", "a".repeat(60));
  assert_eq!(codes(&long, &settings), ["branch-length"]);
  let strict = Settings {
    require_ref: true,
    ..Settings::default()
  };
  assert_eq!(codes("feat/retry", &strict), ["branch-ref"]);
  assert_eq!(codes("feat/12-retry", &strict), Vec::<&str>::new());
}

#[test]
fn the_example_keeps_the_type_reference_and_words() {
  let settings = Settings::default();
  assert_eq!(
    example_branch("feat/Add_Thing", &settings),
    "feat/add-thing"
  );
  assert_eq!(
    example_branch("feature/ENG-4 Fix Login", &settings),
    "<type>/ENG-4-fix-login"
  );
  assert_eq!(example_branch("fix/12-Retry", &settings), "fix/12-retry");
  assert_eq!(example_branch("my_branch", &settings), "<type>/my-branch");
  assert_eq!(
    example_branch("feat/!!", &settings),
    "feat/<short-description>"
  );
}
