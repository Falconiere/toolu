use super::parse;

#[test]
fn ids_lose_their_marketplace_and_enabled_defaults_to_true() {
  let payload = r#"[
    {"id":"rust-quality@toolu","version":"7.11.0","installPath":"/x"},
    {"id":"a@b@toolu","version":"1.0.0","enabled":false},
    {"id":"@lead","version":"2.0.0"},
    {"id":"bare","version":"3.0.0"}
  ]"#;
  let Ok(list) = parse(payload) else {
    panic!("the listing did not parse");
  };
  let names: Vec<(&str, &str, bool)> = list
    .iter()
    .map(|entry| (entry.name.as_str(), entry.version.as_str(), entry.enabled))
    .collect();
  assert_eq!(
    names,
    [
      ("rust-quality", "7.11.0", true),
      ("a@b", "1.0.0", false),
      ("@lead", "2.0.0", true),
      ("bare", "3.0.0", true)
    ]
  );
}

#[test]
fn missing_fields_and_bad_json_are_errors() {
  assert!(parse(r#"[{"version":"1"}]"#).is_err_and(|err| err.contains("\"id\"")));
  assert!(parse(r#"[{"id":"x"}]"#).is_err_and(|err| err.contains("\"version\"")));
  assert!(parse("not json").is_err());
  assert_eq!(parse("[]"), Ok(Vec::new()));
}

#[test]
fn only_toolu_rows_survive_and_the_user_scope_row_wins_over_project_pins() {
  let listing = crate::support::claude_case("list")["stdout"].to_string();
  let Ok(list) = parse(&listing) else {
    panic!("the captured listing did not parse");
  };
  let names: Vec<(&str, &str)> = list
    .iter()
    .map(|entry| (entry.name.as_str(), entry.version.as_str()))
    .collect();
  assert_eq!(
    names,
    [
      ("ast-grep", "8.5.0"),
      ("context7", "7.11.0"),
      ("exa-search", "7.11.0"),
      ("jev", "8.5.0"),
      ("toolu", "8.5.0")
    ]
  );
}

#[test]
fn a_foreign_row_without_a_version_does_not_break_the_listing() {
  let Ok(list) = parse(r#"[{"id":"x@other"},{"id":"jev@toolu","version":"1"}]"#) else {
    panic!("a foreign row broke the listing");
  };
  assert_eq!(
    list
      .iter()
      .map(|entry| entry.name.as_str())
      .collect::<Vec<_>>(),
    ["jev"]
  );
}

fn stepped(code: i32, stdout: &str, stderr: &str) -> (crate::step::Status, String) {
  let ran = crate::host::run::Ran {
    code,
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  };
  let step = super::update_step("jev", &ran, Vec::new());
  (step.status, step.detail)
}

#[test]
fn an_update_without_a_result_line_reads_the_exit_code_and_names_an_old_claude() {
  use crate::step::Status;
  assert_eq!(stepped(0, "", ""), (Status::Updated, "updated".to_owned()));
  assert_eq!(
    stepped(1, "", "\n  boom: network\n"),
    (Status::Failed, "boom: network".to_owned())
  );
  assert_eq!(
    stepped(1, "", "error: unknown option '--json'\n"),
    (
      Status::Failed,
      "error: unknown option '--json'; toolu needs Claude Code 2.1.311 or newer (`claude plugin update --json`)".to_owned()
    )
  );
}

#[test]
fn unknown_or_missing_versions_and_an_unknown_outcome_still_read_sensibly() {
  use crate::step::Status;
  let line = |fields: &str| format!(r#"{{"command":"update","outcome":"ok",{fields}}}"#);
  assert_eq!(
    stepped(
      0,
      &line(r#""updateOutcome":"updated","oldVersion":"unknown","newVersion":"8.6.0""#),
      ""
    ),
    (Status::Updated, "updated to 8.6.0".to_owned())
  );
  assert_eq!(
    stepped(
      0,
      &line(r#""updateOutcome":"updated","newVersion":"unknown""#),
      ""
    ),
    (Status::Updated, "updated".to_owned())
  );
  assert_eq!(
    stepped(0, &line(r#""updateOutcome":"up_to_date""#), ""),
    (Status::Current, "current".to_owned())
  );
  assert_eq!(
    stepped(0, &line(r#""updateOutcome":"something_new""#), ""),
    (Status::Skew, "skipped".to_owned())
  );
  assert_eq!(
    stepped(
      1,
      &line(r#""updateOutcome":"up_to_date","message":"locked""#),
      ""
    ),
    (Status::Failed, "locked".to_owned())
  );
}
