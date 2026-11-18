use super::{official_marketplace, parse};

#[test]
fn grok_keeps_foreign_marketplaces_distinct() {
  let list = r#"[
    {"status":"installed","name":"toolu","version":"10.0.0","marketplace":"toolu"},
    {"status":"installed","name":"toolu","version":"10.0.0","marketplace":null}
  ]"#;
  let entries = parse(list).unwrap();
  assert_eq!(entries[0].source.as_deref(), Some("toolu"));
  assert_eq!(entries[1].source, None);
  let markets = r#"[
    {"name":"toolu","kind":"local","source":{"path":"/other/toolu"}},
    {"name":"external","kind":"git","source":{"url":"https://example.test/toolu.git"}}
  ]"#;
  assert_eq!(official_marketplace(markets).unwrap(), Some(false));
}
