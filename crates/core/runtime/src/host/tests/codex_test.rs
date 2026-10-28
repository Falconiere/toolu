use super::marketplace_pin_notice;

const UNPINNED: &str = r#"
[marketplaces.toolu]
source_type = "git"
source = "https://github.com/Falconiere/toolu.git"
"#;

#[test]
fn a_marketplace_pinned_to_the_tag_is_silent() {
  let config = format!("{UNPINNED}ref = \"v8.6.0\"\n");
  assert_eq!(marketplace_pin_notice(&config, "8.6.0"), None);
}

#[test]
fn a_wrong_ref_names_the_update_command() {
  let config = format!("{UNPINNED}ref = \"v8.5.0\"\n");
  let notice = marketplace_pin_notice(&config, "8.6.0").expect("wrong ref");
  assert!(notice.contains("v8.5.0"), "{notice}");
  assert!(notice.contains("v8.6.0"), "{notice}");
  assert!(
    notice.contains("toolu plugins update --host codex"),
    "{notice}"
  );
}

#[test]
fn an_unpinned_default_branch_warns() {
  let notice = marketplace_pin_notice(UNPINNED, "8.6.0").expect("default branch");
  assert!(notice.contains("default branch"), "{notice}");
}

#[test]
fn a_foreign_marketplace_is_ignored() {
  let config = r#"
[marketplaces.toolu]
source_type = "git"
source = "https://github.com/other/toolu.git"
"#;
  assert_eq!(marketplace_pin_notice(config, "8.6.0"), None);
}
