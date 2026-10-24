use super::{Source, marketplace_list, marketplace_remove, marketplace_source, parse};

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
fn a_plugin_of_another_marketplace_is_not_toolus() {
  let payload = r#"{"installed":[
    {"name":"gmail","marketplaceName":"openai-curated-remote","version":"0.1.10"},
    {"name":"jev","marketplaceName":"comemory","version":"1.0.0"},
    {"name":"jev","marketplaceName":"toolu","version":"8.2.0"}
  ]}"#;
  let Ok(list) = parse(payload) else {
    panic!("the listing did not parse");
  };
  assert_eq!(list.len(), 1);
  assert_eq!(
    (list[0].name.as_str(), list[0].version.as_str()),
    ("jev", "8.2.0")
  );
}

#[test]
fn the_wrong_shape_is_an_error() {
  assert!(parse("[]").is_err_and(|err| err.contains("\"installed\"")));
  assert!(parse(r#"{"installed":[{"name":"x"}]}"#).is_err_and(|err| err.contains("\"version\"")));
  assert!(parse("{").is_err());
}

/// `codex plugin marketplace list --json` as Codex 0.164.0 printed it here.
const MARKETPLACES: &str = r#"{
  "marketplaces": [
    {"name": "openai-bundled", "root": "/h/.codex/.tmp/bundled-marketplaces/openai-bundled",
     "marketplaceSource": {"sourceType": "local", "source": "/h/.codex/.tmp/bundled-marketplaces/openai-bundled"}},
    {"name": "toolu", "root": "/h/.codex/.tmp/marketplaces/toolu",
     "marketplaceSource": {"sourceType": "git", "source": "https://github.com/Falconiere/toolu.git"}}
  ]
}"#;

#[test]
fn a_marketplace_source_is_read_by_name() {
  assert_eq!(
    marketplace_source(MARKETPLACES, "toolu"),
    Ok(Some(Source {
      kind: "git".to_owned(),
      source: "https://github.com/Falconiere/toolu.git".to_owned(),
    }))
  );
  assert_eq!(
    marketplace_source(MARKETPLACES, "openai-bundled").map(|found| found.map(|s| s.kind)),
    Ok(Some("local".to_owned()))
  );
  assert_eq!(marketplace_source(MARKETPLACES, "comemory"), Ok(None));
}

#[test]
fn a_marketplace_listing_of_the_wrong_shape_is_an_error() {
  assert!(marketplace_source("[]", "toolu").is_err_and(|err| err.contains("\"marketplaces\"")));
  assert!(
    marketplace_source(r#"{"marketplaces":[{"name":"toolu"}]}"#, "toolu")
      .is_err_and(|err| err.contains("\"marketplaceSource\""))
  );
  assert!(
    marketplace_source(
      r#"{"marketplaces":[{"name":"toolu","marketplaceSource":{"sourceType":"git"}}]}"#,
      "toolu"
    )
    .is_err_and(|err| err.contains("\"source\""))
  );
}

#[test]
fn marketplace_commands_name_codex_and_the_toolu_marketplace() {
  assert_eq!(
    marketplace_list().join(" "),
    "codex plugin marketplace list --json"
  );
  assert_eq!(
    marketplace_remove().join(" "),
    "codex plugin marketplace remove toolu"
  );
}
