//! The semantic commit message: subject parsing, git's trailer block, every
//! check, and the corrected example.

use super::{
  Subject, check_message, example_message, is_wip, subject, trailer, trailers, with_trailers,
};
use crate::settings::{Format, Settings};

const GOOD: &str = "feat(git-better): block commits without an intent\n\nBody text.\n\nIntent: agents learn why a commit exists\nRisk: low\nRefs: #740";

fn codes(message: &str) -> Vec<&'static str> {
  codes_with(message, &Settings::default(), false)
}

fn codes_with(message: &str, settings: &Settings, on_base: bool) -> Vec<&'static str> {
  check_message(message, settings, on_base)
    .into_iter()
    .map(|violation| violation.code)
    .collect()
}

#[test]
fn a_subject_has_a_type_an_optional_scope_and_bang_and_a_description() {
  assert_eq!(
    subject("feat(shell/git): read x"),
    Some(Subject {
      kind: "feat",
      scope: Some("shell/git"),
      breaking: false,
      description: "read x",
    })
  );
  let bang = subject("refactor!: drop y").unwrap();
  assert_eq!((bang.scope, bang.breaking), (None, true));
  for line in [
    "Feat: x",
    "feat x",
    "feat:x",
    "feat(): x",
    "feat(Shell): x",
    "feat(x: y",
    "feat: ",
    "(x): y",
  ] {
    assert_eq!(subject(line), None, "{line}");
  }
}

#[test]
fn trailers_are_the_last_paragraph_when_every_line_is_one() {
  let found = trailers(GOOD);
  assert_eq!(
    trailer(&found, "intent"),
    Some("agents learn why a commit exists")
  );
  assert_eq!(trailer(&found, "Risk"), Some("low"));
  let folded = trailers("fix(x): y\n\nIntent: one\n  two\nRefs: #1");
  assert_eq!(trailer(&folded, "Intent"), Some("one two"));
  assert_eq!(trailers("fix(x): y"), []);
  assert_eq!(trailers("fix(x): y\n\nIntent: why\nfree text"), []);
  assert_eq!(trailers("fix(x): y\n\nIntent: why\nBREAKING CHANGE: z"), []);
}

#[test]
fn extra_trailers_join_the_block_or_start_one() {
  assert_eq!(
    with_trailers("fix(x): y\n", &["Intent: why".to_owned()]),
    "fix(x): y\n\nIntent: why"
  );
  assert_eq!(
    with_trailers("fix(x): y\n\nRisk: low", &["Intent: why".to_owned()]),
    "fix(x): y\n\nRisk: low\nIntent: why"
  );
  assert_eq!(with_trailers("fix(x): y\n\n", &[]), "fix(x): y");
}

#[test]
fn a_semantic_message_passes() {
  assert_eq!(codes(GOOD), Vec::<&str>::new());
  let breaking =
    "feat(cli)!: drop the old flag\n\nIntent: one way to do it\nBREAKING-CHANGE: --old is gone";
  assert_eq!(codes(breaking), Vec::<&str>::new());
}

#[test]
fn each_check_names_what_is_wrong() {
  let cases: [(&str, &[&str]); 12] = [
    ("", &["empty"]),
    ("fix stuff", &["subject-format", "missing-intent"]),
    ("wibble(x): y\n\nIntent: z", &["unknown-type"]),
    ("fix: y\n\nIntent: z", &["missing-scope"]),
    ("fix(x): Add y\n\nIntent: z", &["subject-case"]),
    ("fix(x): add y.\n\nIntent: z", &["subject-period"]),
    ("fix(x): y\nbody\n\nIntent: z", &["body-spacing"]),
    ("fix(x): y", &["missing-intent"]),
    ("fix(x): y\n\nIntent:", &["missing-intent"]),
    ("fix(x): y\n\nIntent: z\nRisk: huge", &["risk-value"]),
    ("feat(x)!: y\n\nIntent: z", &["breaking-note"]),
    (
      "feat(x): y\n\nIntent: z\nBREAKING-CHANGE: w",
      &["breaking-bang"],
    ),
  ];
  for (message, expected) in cases {
    assert_eq!(codes(message), expected, "{message:?}");
  }
  let long = format!("feat(x): {}\n\nIntent: z", "a".repeat(70));
  assert_eq!(codes(&long), ["subject-length"]);
  let spaced = "feat(x)!: y\n\nIntent: z\nBREAKING CHANGE: w";
  assert_eq!(codes(spaced), ["breaking-spelling"]);
}

#[test]
fn an_intent_outside_the_trailer_paragraph_is_named() {
  let split = "feat(x): y\n\nIntent: z\n\nCo-Authored-By: A <a@example.com>";
  assert_eq!(codes(split), ["intent-placement"]);
  let joined = "feat(x): y\n\nIntent: z\nCo-Authored-By: A <a@example.com>";
  assert_eq!(codes(joined), Vec::<&str>::new());
}

#[test]
fn autosquash_subjects_always_pass_and_wip_passes_off_the_base_branch() {
  for message in ["fixup! feat(x): y", "squash! fix(y): z", "amend! fix(y): z"] {
    assert_eq!(codes(message), Vec::<&str>::new(), "{message}");
  }
  let settings = Settings::default();
  assert_eq!(
    codes_with("wip: half done", &settings, false),
    Vec::<&str>::new()
  );
  assert_eq!(codes_with("WIP", &settings, false), Vec::<&str>::new());
  assert_eq!(codes_with("wip: half done", &settings, true), ["wip"]);
  let strict = Settings {
    format: Format {
      wip: false,
      ..Format::default()
    },
    ..Settings::default()
  };
  assert_eq!(codes_with("wip(x): y", &strict, false), ["wip"]);
  assert!(is_wip("wip!: y") && !is_wip("wipe(x): y"));
}

#[test]
fn settings_change_what_is_required() {
  let loose = Settings {
    format: Format {
      scope: false,
      ..Format::default()
    },
    types: vec!["feat".to_owned(), "wibble".to_owned()],
    ..Settings::default()
  };
  assert_eq!(
    codes_with("wibble: y\n\nIntent: z", &loose, false),
    Vec::<&str>::new()
  );
  assert_eq!(
    codes_with("fix: y\n\nIntent: z", &loose, false),
    ["unknown-type"]
  );
}

#[test]
fn the_example_keeps_the_subject_and_adds_what_is_missing() {
  assert_eq!(
    example_message("fix(cli): Handle a missing flag."),
    "fix(cli): handle a missing flag\n\nIntent: <why this change is needed>"
  );
  assert_eq!(
    example_message("feat!: drop it"),
    "feat(<area>)!: drop it\n\nIntent: <why this change is needed>\nBREAKING-CHANGE: <what breaks and how to migrate>"
  );
  assert_eq!(
    example_message("fix(cli): Keep it\n\nIntent: users lose work"),
    "fix(cli): keep it\n\nIntent: users lose work"
  );
  assert_eq!(
    example_message("fixed stuff"),
    "<type>(<area>): <what changed>\n\nIntent: <why this change is needed>"
  );
}
