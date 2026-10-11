use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{dependency_specs, dependency_warning};

#[test]
fn specs_follow_jq_and_an_unknown_record_silences_the_warning() {
  let manifest = serde_json::json!({
    "dependencies": [
      {"name": "alpha", "marketplace": false},
      {"name": "beta", "marketplace": "toolu"},
      "gamma@toolu\n",
      "",
      {"name": 1},
      "a\nb"
    ]
  });
  assert_eq!(
    dependency_specs(&manifest),
    vec!["alpha", "beta@toolu", "gamma@toolu", "a", "b"]
  );
  assert_eq!(dependency_specs(&Value::Null), Vec::<String>::new());

  let dir = tempfile::tempdir().unwrap();
  let plugin = dir.path().join("plugin");
  let claude = plugin.join(".claude-plugin");
  std::fs::create_dir_all(&claude).unwrap();
  std::fs::write(
    claude.join("plugin.json"),
    r#"{"dependencies":["alpha@toolu"]}"#,
  )
  .unwrap();
  let root = plugin.to_str().unwrap();
  let bare = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", dir.path().join("cfg").to_str().unwrap()),
  ]);
  assert_eq!(
    dependency_warning(&bare, Host::Claude, root, dir.path().to_str().unwrap()),
    None
  );

  let cfg = dir.path().join("cfg");
  let plugins = cfg.join("plugins");
  std::fs::create_dir_all(&plugins).unwrap();
  std::fs::write(plugins.join("installed_plugins.json"), r#"{"plugins":{}}"#).unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", cfg.to_str().unwrap()),
  ]);
  let warning = dependency_warning(&env, Host::Opencode, root, "").unwrap();
  assert!(warning.contains("/plugin install alpha@toolu"));
  std::fs::write(
    plugins.join("installed_plugins.json"),
    r#"{"plugins":{"alpha@toolu":{}}}"#,
  )
  .unwrap();
  assert_eq!(dependency_warning(&env, Host::Claude, root, ""), None);
}

#[test]
fn codex_prefers_its_manifest_only_when_that_file_lists_dependencies() {
  let dir = tempfile::tempdir().unwrap();
  let plugin = dir.path().join("plugin");
  std::fs::create_dir_all(plugin.join(".codex-plugin")).unwrap();
  std::fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
  std::fs::write(
    plugin.join(".codex-plugin").join("plugin.json"),
    r#"{"name":"toolu"}"#,
  )
  .unwrap();
  std::fs::write(
    plugin.join(".claude-plugin").join("plugin.json"),
    r#"{"dependencies":["beta@toolu"]}"#,
  )
  .unwrap();
  let cfg = dir.path().join("cfg");
  std::fs::create_dir_all(cfg.join("toolu")).unwrap();
  std::fs::write(
    cfg.join("toolu").join("codex-plugins.json"),
    r#"{"version":1,"status":"ready","plugins":[]}"#,
  )
  .unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", cfg.to_str().unwrap()),
  ]);
  let warning = dependency_warning(&env, Host::Codex, plugin.to_str().unwrap(), "").unwrap();
  assert!(warning.contains("codex plugin add beta@toolu"));
}
