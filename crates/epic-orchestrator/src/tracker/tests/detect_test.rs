use toolu_runtime::env::Env;

use super::{LinearRef, TrackerKind, detect_tracker, parse_jira_ref, parse_linear_ref};

fn none() -> Env {
  Env::from_pairs([("HOME", "/nonexistent")])
}

#[test]
fn github_refs_are_github() {
  for reference in ["https://github.com/a/b/issues/3", "a/b#3", "#3", "3"] {
    assert_eq!(
      detect_tracker(reference, None, &none()),
      Ok(TrackerKind::Github)
    );
  }
}

#[test]
fn urls_and_prefixes_are_unambiguous() {
  let cases = [
    ("https://acme.atlassian.net/browse/PAY-7", TrackerKind::Jira),
    ("jira:PAY-7", TrackerKind::Jira),
    (
      "https://linear.app/acme/issue/ENG-9/title",
      TrackerKind::Linear,
    ),
    ("linear:ENG-9", TrackerKind::Linear),
  ];
  for (reference, kind) in cases {
    assert_eq!(
      detect_tracker(reference, None, &none()),
      Ok(kind),
      "{reference}"
    );
  }
}

#[test]
fn a_bare_key_follows_the_configured_tracker_else_asks() {
  let linear = none().with("LINEAR_API_KEY", "k");
  assert_eq!(
    detect_tracker("PAY-7", None, &linear),
    Ok(TrackerKind::Linear)
  );
  let jira = none().with("JIRA_BASE_URL", "https://x");
  assert_eq!(detect_tracker("PAY-7", None, &jira), Ok(TrackerKind::Jira));
  let both = linear.with("JIRA_BASE_URL", "https://x");
  let err = detect_tracker("PAY-7", None, &both).expect_err("ambiguous");
  assert!(err.contains("prefix it"), "{err}");
  assert_eq!(
    detect_tracker("PAY-7", Some("jira"), &none()),
    Ok(TrackerKind::Jira)
  );
  assert!(detect_tracker("PAY-7", Some("gitlab"), &none()).is_err());
  let err = detect_tracker("what?", None, &none()).expect_err("unknown");
  assert_eq!(err, "unrecognized epic reference: \"what?\"");
}

#[test]
fn jira_keys_and_browse_urls_parse() {
  assert_eq!(
    parse_jira_ref("https://acme.atlassian.net/browse/PAY-12?focused=1").as_deref(),
    Some("PAY-12")
  );
  assert_eq!(parse_jira_ref("jira:pay-12").as_deref(), Some("PAY-12"));
  assert_eq!(parse_jira_ref("12"), None);
  assert_eq!(parse_jira_ref("1AB-2"), None);
}

#[test]
fn linear_issues_and_projects_parse() {
  assert_eq!(
    parse_linear_ref("https://linear.app/acme/issue/ENG-42/fix-login"),
    Some(LinearRef::Issue("ENG-42".into()))
  );
  assert_eq!(
    parse_linear_ref("https://linear.app/acme/project/sync-engine-4f2a9c1b8e7d"),
    Some(LinearRef::Project("4f2a9c1b8e7d".into()))
  );
  assert_eq!(
    parse_linear_ref("project:abc123"),
    Some(LinearRef::Project("abc123".into()))
  );
  assert_eq!(
    parse_linear_ref("https://linear.app/acme/project/sync-4f2a9c1b8e7d-/"),
    Some(LinearRef::Project("4f2a9c1b8e7d".into()))
  );
  assert_eq!(parse_linear_ref("https://linear.app/acme/project/-"), None);
}

#[test]
fn kind_names_round_trip() {
  for kind in [TrackerKind::Github, TrackerKind::Jira, TrackerKind::Linear] {
    assert_eq!(TrackerKind::parse(kind.name()), Some(kind));
  }
  assert_eq!(TrackerKind::parse("gitlab"), None);
}
