//! The `gitBetter` section: defaults, every key, values that are ignored, and
//! the type list kept in step with `commit-prefixes.txt`.

use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::config::settings::read_list;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{DEFAULT_TYPES, Mode, Settings, load};

/// The settings for a project whose `.claude/toolu.config.json` is `config`.
fn loaded(config: Option<&str>) -> Settings {
  let dir = tempfile::tempdir().unwrap();
  let project = dir.path().join("project");
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  if let Some(config) = config {
    std::fs::write(project.join(".claude/toolu.config.json"), config).unwrap();
  }
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    (
      "TOOLU_CONFIG_DIR",
      dir.path().join("user").display().to_string(),
    ),
    ("TOOLU_PROJECT_DIR", project.display().to_string()),
    (
      "TOOLU_SETTINGS_DIR",
      dir.path().join("settings").display().to_string(),
    ),
  ]);
  let roots = Roots::new(env, Some(Host::Claude));
  load(&roots, Some(&project), None)
}

#[test]
fn the_defaults_block_and_require_a_scope() {
  assert_eq!(loaded(None), Settings::default());
  let defaults = Settings::default();
  assert_eq!(defaults.mode, Mode::Block);
  assert!(defaults.require_scope && defaults.allow_wip && !defaults.require_ref);
}

#[test]
fn every_key_is_read() {
  let settings = loaded(Some(
    r#"{"version":1,"gitBetter":{"mode":"advise","requireScope":false,"requireRef":true,"allowWip":false,"types":["feat","fix"]}}"#,
  ));
  assert_eq!(
    settings,
    Settings {
      mode: Mode::Advise,
      require_scope: false,
      require_ref: true,
      allow_wip: false,
      types: vec!["feat".to_owned(), "fix".to_owned()],
      notes: Vec::new(),
    }
  );
  let off = loaded(Some(r#"{"version":1,"gitBetter":{"mode":"off"}}"#));
  assert_eq!(off.mode, Mode::Off);
}

#[test]
fn a_value_that_cannot_be_read_keeps_its_default_and_is_reported() {
  let settings = loaded(Some(
    r#"{"version":1,"gitBetter":{"mode":"loud","requireScope":"yes","types":["Feat"]}}"#,
  ));
  assert_eq!(settings.mode, Mode::Block);
  assert!(settings.require_scope);
  assert_eq!(settings.types, Settings::default().types);
  assert_eq!(settings.notes.len(), 3, "{:?}", settings.notes);
  let shape = loaded(Some(r#"{"version":1,"gitBetter":true}"#));
  assert_eq!(shape.mode, Mode::Block);
  assert_eq!(
    shape.notes,
    ["gitBetter must be an object; its defaults apply"]
  );
}

#[test]
fn an_invalid_config_keeps_enforcing_the_defaults() {
  let settings = loaded(Some(r#"{"version":1,"unknownKey":1}"#));
  assert_eq!(settings.mode, Mode::Block);
  assert_eq!(settings.notes.len(), 1, "{:?}", settings.notes);
}

#[test]
fn the_types_follow_commit_prefixes_txt() {
  let dir = tempfile::tempdir().unwrap();
  let settings_dir = dir.path().join("settings");
  std::fs::create_dir_all(&settings_dir).unwrap();
  std::fs::write(
    settings_dir.join("commit-prefixes.txt"),
    "# types\nfeat\nwip\n",
  )
  .unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    (
      "TOOLU_CONFIG_DIR",
      dir.path().join("user").display().to_string(),
    ),
    ("TOOLU_SETTINGS_DIR", settings_dir.display().to_string()),
  ]);
  let roots = Roots::new(env, Some(Host::Claude));
  let settings = load(&roots, Some(dir.path()), None);
  assert_eq!(settings.types, ["feat", "wip"]);
}

#[test]
fn the_default_types_match_the_shipped_commit_prefixes() {
  let shipped =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu/settings/commit-prefixes.txt");
  assert_eq!(read_list(&shipped).unwrap(), DEFAULT_TYPES);
}
