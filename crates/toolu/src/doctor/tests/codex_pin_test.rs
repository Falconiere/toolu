use toolu_runtime::env::Env;

use super::{notice, pin_notice};

/// `[marketplaces.toolu]` as `codex plugin marketplace add Falconiere/toolu`
/// wrote it on this machine (Codex 0.164.0), with neighbours.
const UNPINNED: &str = r#"model = "gpt-6-sol"

[marketplaces.toolu]
source_type = "git"
source = "https://github.com/Falconiere/toolu.git"

[marketplaces.comemory]
source_type = "local"
source = "/Users/u/.comemory/integrations"

[plugins."toolu@toolu"]
enabled = true
"#;

fn pinned(tag: &str) -> String {
  UNPINNED.replace(
    "source = \"https://github.com/Falconiere/toolu.git\"\n",
    &format!("source = \"https://github.com/Falconiere/toolu.git\"\nref = \"{tag}\"\n"),
  )
}

#[test]
fn an_unpinned_marketplace_names_the_default_branch_and_the_fix() {
  assert_eq!(
    pin_notice(UNPINNED, "8.2.0").as_deref(),
    Some(
      "the Codex marketplace toolu follows its default branch, so Codex asks to review toolu's hooks again after every change to it; toolu plugins update --host codex"
    )
  );
}

#[test]
fn another_tag_names_both_tags() {
  assert_eq!(
    pin_notice(&pinned("v8.1.4"), "8.2.0").as_deref(),
    Some(
      "the Codex marketplace toolu is pinned to v8.1.4, not v8.2.0; toolu plugins update --host codex"
    )
  );
}

#[test]
fn this_binarys_tag_says_nothing() {
  assert_eq!(pin_notice(&pinned("v8.2.0"), "8.2.0"), None);
}

#[test]
fn a_foreign_absent_or_broken_marketplace_says_nothing() {
  let local = "[marketplaces.toolu]\nsource_type = \"local\"\nsource = \"/src/toolu\"\n";
  assert_eq!(pin_notice(local, "8.2.0"), None);
  assert_eq!(pin_notice("model = \"x\"\n", "8.2.0"), None);
  assert_eq!(pin_notice("[marketplaces.toolu\n", "8.2.0"), None);
}

#[test]
fn the_config_is_read_from_codex_home() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("config.toml"), UNPINNED).unwrap();
  let home = dir.path().display().to_string();
  let env = Env::from_pairs([("CODEX_HOME", home.as_str())]);
  assert!(notice(&env, "8.2.0").is_some_and(|text| text.contains("default branch")));
  let empty = tempfile::tempdir().unwrap();
  let missing = empty.path().display().to_string();
  let env = Env::from_pairs([("CODEX_HOME", missing.as_str())]);
  assert_eq!(notice(&env, "8.2.0"), None);
}

#[test]
fn toolus_origin_is_recognized_however_it_is_spelled_and_others_are_not() {
  let spelled = UNPINNED.replace(
    "https://github.com/Falconiere/toolu.git",
    "https://github.com/falconiere/toolu/",
  );
  assert!(pin_notice(&spelled, "8.2.0").is_some_and(|text| text.contains("default branch")));
  let fork = UNPINNED.replace(
    "https://github.com/Falconiere/toolu.git",
    "https://github.com/someone/toolu.git",
  );
  assert_eq!(pin_notice(&fork, "8.2.0"), None);
}

#[test]
fn a_blank_codex_home_falls_back_to_home_dot_codex() {
  let home = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(home.path().join(".codex")).unwrap();
  std::fs::write(home.path().join(".codex/config.toml"), pinned("v8.1.4")).unwrap();
  let home_text = home.path().display().to_string();
  let env = Env::from_pairs([("HOME", home_text.as_str()), ("CODEX_HOME", " ")]);
  assert!(notice(&env, "8.2.0").is_some_and(|text| text.contains("pinned to v8.1.4")));
}
