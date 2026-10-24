use super::{BUILT_IN, plugin_names};

#[test]
fn the_built_in_catalog_names_the_shipped_plugins_and_not_the_retired_ones() {
  let names = plugin_names(BUILT_IN);
  assert_eq!(names.first().map(String::as_str), Some("toolu"));
  assert!(names.iter().any(|name| name == "jev"), "{names:?}");
  for retired in ["context7", "exa-search", "jira", "agent-browser"] {
    assert!(!names.iter().any(|name| name == retired), "{retired}");
  }
}

#[test]
fn text_that_is_not_a_manifest_names_no_plugin() {
  assert_eq!(plugin_names("{"), Vec::<String>::new());
  assert_eq!(plugin_names(r#"{"plugins":3}"#), Vec::<String>::new());
  assert_eq!(
    plugin_names(r#"{"plugins":[{"name":"a"},{"name":7},{}]}"#),
    ["a"]
  );
}
