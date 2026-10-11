use std::os::unix::fs::PermissionsExt;

use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::env::Env;

use super::{mandate_block, missing_tools_warning};

#[test]
fn the_mandate_needs_the_tool_and_the_warning_needs_its_absence() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let config = LoadedConfig::from_data(Map::new(), Host::Claude);
  let missing = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("PATH", bin.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap()),
  ]);
  let warning = missing_tools_warning(&missing, &config).unwrap();
  assert!(warning.contains("ast-grep (structural code search)"));
  assert_eq!(mandate_block(&config, &missing, Host::Claude), None);
  let sg = bin.join("sg");
  std::fs::write(&sg, "").unwrap();
  let mut perms = std::fs::metadata(&sg).unwrap().permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(&sg, perms).unwrap();
  let found = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("PATH", bin.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap()),
  ]);
  assert_eq!(missing_tools_warning(&found, &config), None);
  let block = mandate_block(&config, &found, Host::Cursor).unwrap();
  assert!(block.contains("ast-grep run --pattern"));
  let plugins = dir.path().join("plugins");
  std::fs::create_dir_all(&plugins).unwrap();
  std::fs::write(plugins.join("installed_plugins.json"), r#"{"plugins":{}}"#).unwrap();
  assert_eq!(mandate_block(&config, &found, Host::Claude), None);
}
