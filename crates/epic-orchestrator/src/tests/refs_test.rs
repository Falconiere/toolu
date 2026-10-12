use std::path::PathBuf;

use toolu_runtime::env::Env;

use super::{branch_for, epics_home, issue_key, key_for, parse_ref, refs_in, slugify};

fn parts(text: &str, default: Option<&str>) -> (String, String, u64) {
  let parsed = parse_ref(text, default).expect("ref");
  (parsed.owner, parsed.repo, parsed.number)
}

#[test]
fn parse_ref_reads_urls_short_forms_and_bare_numbers() {
  assert_eq!(
    parts("https://github.com/Falconiere/comemory/issues/248", None),
    ("Falconiere".into(), "comemory".into(), 248)
  );
  assert_eq!(
    parts("CodaSignal/comemory.io#183", None),
    ("CodaSignal".into(), "comemory.io".into(), 183)
  );
  assert_eq!(parts("#12", Some("a/b")), ("a".into(), "b".into(), 12));
  assert_eq!(parts(" 7 ", Some("a/b")), ("a".into(), "b".into(), 7));
  assert_eq!(
    parts("https://github.com/o/r/pull/9", None),
    ("o".into(), "r".into(), 9)
  );
}

#[test]
fn parse_ref_refuses_text_and_a_malformed_default() {
  let err = parse_ref("not a ref", Some("a/b")).expect_err("text");
  assert_eq!(err, "unrecognized issue reference: \"not a ref\"");
  let err = parse_ref("#3", Some("nobody")).expect_err("default");
  assert_eq!(err, "bad default_repo: nobody");
  assert!(parse_ref("#3", Some("/b")).is_err());
}

#[test]
fn refs_in_matches_checklist_forms_but_not_a_number_after_a_name() {
  let line = "- [ ] CodaSignal/comemory.io#183 Drain durable push";
  assert_eq!(
    refs_in(line, "Falconiere/comemory").expect("refs"),
    vec!["CodaSignal/comemory.io#183".to_owned()]
  );
  let mixed = "blocked by #4 and https://github.com/o/r/issues/5, not issue#6 or path/#7";
  assert_eq!(
    refs_in(mixed, "a/b").expect("refs"),
    vec!["a/b#4".to_owned(), "o/r#5".to_owned()]
  );
}

#[test]
fn issue_key_is_a_valid_herdr_agent_name() {
  let valid = regex::Regex::new("^[a-z][a-z0-9_-]{0,31}$").expect("pattern");
  for (repo, number) in [
    ("comemory.io", 183),
    ("homebrew-tap", 1),
    ("9lives", 3),
    ("an-extremely-long-repository-name-for-testing", 12_345),
    ("Falconiere/comemory", 255),
    ("", 4),
  ] {
    let key = issue_key(repo, number);
    assert!(valid.is_match(&key), "{key}");
    assert!(key.ends_with(&format!("-{number}")), "{key}");
  }
  assert_eq!(issue_key("comemory.io", 183), "comemory-io-183");
  assert_eq!(
    issue_key("Falconiere/comemory", 255),
    "falconiere-comemory-255"
  );
  assert_eq!(issue_key("9lives", 3), "r9lives-3");
  assert_eq!(issue_key("", 4), "repo-4");
}

#[test]
fn issue_key_folds_the_owner_so_same_name_repos_do_not_collide() {
  assert_ne!(issue_key("orgA/foo", 1), issue_key("orgB/foo", 1));
}

#[test]
fn slugify_drops_the_epic_prefix() {
  assert_eq!(
    slugify(
      "[Realtime replication] Drain durable push/pull backlogs safely",
      5
    ),
    "drain-durable-push-pull-backlogs"
  );
  assert_eq!(slugify("[only prefix]", 5), "work");
  assert_eq!(slugify("Ünïcode títle 2", 5), "n-code-t-tle-2");
}

#[test]
fn keys_and_branches_for_jira_and_linear_items_are_valid_herdr_names() {
  assert_eq!(key_for("PAY-12", "acme/pay", None), "pay-12");
  assert_eq!(
    branch_for("PAY-12", None, "Add refunds API"),
    "feat/pay-12-add-refunds-api"
  );
  assert_eq!(key_for("a/b#3", "acme/pay", Some(3)), "acme-pay-3");
  assert_eq!(branch_for("a/b#3", Some(3), "Fix it"), "feat/3-fix-it");
}

#[test]
fn epics_home_follows_the_host_override_and_epic_state_home() {
  let env = |pairs: &[(&str, &str)]| {
    Env::from_pairs(
      pairs
        .iter()
        .copied()
        .chain([("HOME", "/home/u")])
        .map(|(key, value)| (key.to_owned(), value.to_owned())),
    )
  };
  assert_eq!(
    epics_home(&env(&[("EPIC_STATE_HOME", "/tmp/epics")])),
    PathBuf::from("/tmp/epics")
  );
  assert_eq!(
    epics_home(&env(&[
      ("TOOLU_HOST_OVERRIDE", "codex"),
      ("CODEX_HOME", "/c")
    ])),
    PathBuf::from("/c/toolu/epics")
  );
  assert_eq!(
    epics_home(&env(&[
      ("TOOLU_HOST_OVERRIDE", "opencode"),
      ("TOOLU_OPENCODE_HOME", "/o")
    ])),
    PathBuf::from("/o/toolu/epics")
  );
  assert_eq!(
    epics_home(&env(&[("TOOLU_HOST_OVERRIDE", "claude")])),
    PathBuf::from("/home/u/.claude/epics")
  );
  assert_eq!(
    epics_home(&env(&[("CODEX_HOME", "/c")])),
    PathBuf::from("/c/toolu/epics")
  );
  assert_eq!(
    epics_home(&env(&[("CODEX_HOME", "/c"), ("CLAUDE_CONFIG_DIR", "/cl")])),
    PathBuf::from("/home/u/.claude/epics")
  );
  assert_eq!(
    epics_home(&env(&[("OPENCODE_HOME", "/oc")])),
    PathBuf::from("/oc/toolu/epics")
  );
}
