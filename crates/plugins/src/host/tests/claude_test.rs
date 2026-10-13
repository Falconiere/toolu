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
  let names: Vec<(&str, &str, bool)> =
    list.iter().map(|entry| (entry.name.as_str(), entry.version.as_str(), entry.enabled)).collect();
  assert_eq!(
    names,
    [("rust-quality", "7.11.0", true), ("a@b", "1.0.0", false), ("@lead", "2.0.0", true), ("bare", "3.0.0", true)]
  );
}

#[test]
fn missing_fields_and_bad_json_are_errors() {
  assert!(parse(r#"[{"version":"1"}]"#).is_err_and(|err| err.contains("\"id\"")));
  assert!(parse(r#"[{"id":"x"}]"#).is_err_and(|err| err.contains("\"version\"")));
  assert!(parse("not json").is_err());
  assert_eq!(parse("[]"), Ok(Vec::new()));
}
