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
