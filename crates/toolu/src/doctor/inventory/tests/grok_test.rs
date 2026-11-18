#[test]
fn doctor_counts_only_active_toolu_plugins() {
  let report = serde_json::json!({"plugins": [
    {"name":"delivery-flow","path":"/profile/delivery-flow","enabled":true},
    {"name":"toolu","path":"/profile/toolu","enabled":false},
    {"name":"other","path":"/profile/other","enabled":true}
  ]});
  let found = super::parse(&report).unwrap();
  assert_eq!(found.plugins.len(), 1);
  assert_eq!(found.plugins[0].name, "delivery-flow");
  assert_eq!(
    found.plugins[0].root.as_deref(),
    Some(std::path::Path::new("/profile/delivery-flow"))
  );
}
