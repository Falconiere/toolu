use std::path::Path;

use toolu_runtime::env::Env;

use crate::host::Target;
use crate::local::store::{Store, version_of};

#[test]
fn a_qwen_extension_preserves_matchers_and_passes_its_installed_root_as_data() {
  let home = tempfile::tempdir().unwrap();
  let profile = home.path().join("Qwen's profile");
  let env = Env::from_pairs([("QWEN_HOME", profile.to_str().unwrap())]);
  let store = Store::locate(&env, Target::Qwen);
  let root = profile.join("extensions/toolu");
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu");
  store.put(&source, "toolu", "v1").unwrap();
  let manifest: serde_json::Value =
    serde_json::from_slice(&std::fs::read(root.join("qwen-extension.json")).unwrap()).unwrap();
  assert_eq!(manifest["version"], version_of(&source).unwrap());
  let pre_tool = &manifest["hooks"]["PreToolUse"][0];
  assert!(pre_tool["matcher"].as_str().unwrap().contains("Bash"));
  let hook = &pre_tool["hooks"][0];
  assert_eq!(hook["env"]["TOOLU_HOST_OVERRIDE"], "qwen");
  assert_eq!(hook["env"]["TOOLU_PLUGIN_ROOT"], root.to_str().unwrap());
  let launcher = hook["command"].as_str().unwrap();
  assert!(launcher.contains("--host qwen --hook-protocol"));
  assert!(launcher.contains("\"${TOOLU_PLUGIN_ROOT}\""));
  let command = std::fs::read_to_string(root.join("commands/commit.md")).unwrap();
  assert!(command.contains(root.join("workflows/commit.md").to_str().unwrap()));
}
