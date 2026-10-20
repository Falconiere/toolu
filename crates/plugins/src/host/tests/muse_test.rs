use super::{Source, marketplace, parse, pending};
use crate::support::muse_output;

#[test]
fn the_captured_installed_list_reads_each_record_with_its_source() {
  let Ok(list) = parse(&muse_output("list")) else {
    panic!("the captured listing did not parse");
  };
  let rows: Vec<(&str, &str, bool)> = list
    .iter()
    .map(|entry| (entry.name.as_str(), entry.version.as_str(), entry.enabled))
    .collect();
  assert_eq!(rows, [("hooky", "1.0.1", false), ("toolu", "8.0.4", true)]);
  assert_eq!(
    list[1].source.as_deref(),
    Some("/home/user/.local/share/toolu/muse-marketplace/plugins/toolu")
  );
}

#[test]
fn the_captured_available_list_keeps_only_the_toolu_marketplace() {
  let Ok(list) = parse(&muse_output("available")) else {
    panic!("the captured available listing did not parse");
  };
  let names: Vec<(&str, &str)> = list
    .iter()
    .map(|entry| (entry.name.as_str(), entry.version.as_str()))
    .collect();
  assert_eq!(names, [("brainstorm", "8.0.4"), ("toolu", "8.0.4")]);
  assert!(
    list
      .iter()
      .all(|entry| entry.enabled && entry.source.is_none())
  );
}

#[test]
fn a_malformed_listing_is_an_error_never_an_empty_list() {
  assert_eq!(parse(r#"{"plugins":[]}"#), Ok(Vec::new()));
  assert!(parse(r#"{"plugins":{}}"#).is_err_and(|err| err.contains("\"plugins\"")));
  assert!(parse("[]").is_err());
  assert!(
    parse(r#"{"plugins":[{"plugin":{"id":"toolu"}}]}"#)
      .is_err_and(|err| err.contains("\"record\""))
  );
  assert!(
    parse(r#"{"plugins":[{"record":{"id":"toolu"}}]}"#)
      .is_err_and(|err| err.contains("\"version\""))
  );
  assert!(parse(r#"{"available":[{"marketplace":"toolu","name":"x"}]}"#).is_err());
  assert!(parse("{").is_err());
}

#[test]
fn inspect_is_pending_for_review_needed_and_modified_only() {
  assert_eq!(pending(&muse_output("inspect-review-needed")), Ok(true));
  assert_eq!(pending(&muse_output("inspect-modified")), Ok(true));
  assert_eq!(pending(&muse_output("inspect-trusted")), Ok(false));
  assert_eq!(pending(&muse_output("inspect-toolu-trusted")), Ok(false));
  assert_eq!(pending(r#"{"runtime_capabilities":[]}"#), Ok(false));
  assert!(pending(r#"{"error":{"code":"unknown-plugin"}}"#).is_err());
  assert!(pending(r#"{"runtime_capabilities":[{}]}"#).is_err());
}

#[test]
fn the_captured_marketplace_list_names_each_source() {
  let listing = muse_output("marketplace-list");
  assert_eq!(
    marketplace(&listing, "toolu"),
    Ok(Some(Source {
      kind: "local-dir".to_owned(),
      path: "/home/user/.local/share/toolu/muse-marketplace".to_owned(),
    }))
  );
  assert_eq!(marketplace(&listing, "absent"), Ok(None));
  assert!(marketplace("{}", "toolu").is_err());
  assert!(marketplace(r#"{"marketplaces":[{"name":"toolu"}]}"#, "toolu").is_err());
}
