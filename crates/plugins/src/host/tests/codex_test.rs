use super::parse;

#[test]
fn the_installed_array_is_read_and_enabled_defaults_to_true() {
  let payload = r#"{"installed":[
    {"name":"jev","marketplaceName":"toolu","version":"7.11.0","source":{"path":"/p"}},
    {"name":"ast-grep","version":"7.10.0","enabled":false}
  ]}"#;
  let Ok(list) = parse(payload) else {
    panic!("the listing did not parse");
  };
  assert_eq!(list.len(), 2);
  assert_eq!((list[0].name.as_str(), list[0].enabled), ("jev", true));
  assert_eq!(
    (list[1].version.as_str(), list[1].enabled),
    ("7.10.0", false)
  );
}

#[test]
fn the_wrong_shape_is_an_error() {
  assert!(parse("[]").is_err_and(|err| err.contains("\"installed\"")));
  assert!(parse(r#"{"installed":[{"name":"x"}]}"#).is_err_and(|err| err.contains("\"version\"")));
  assert!(parse("{").is_err());
}
