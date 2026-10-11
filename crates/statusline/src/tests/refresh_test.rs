use std::fs;
use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::refresh;

#[test]
fn migrates_legacy_settings_once_and_preserves_the_backup() {
  let temp = tempfile::tempdir().expect("temp directory");
  let settings = temp.path().join("settings.json");
  let original = r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"},"theme":"dark"}"#;
  fs::write(&settings, original).expect("settings");
  let roots = Roots::new(
    Env::from_pairs([("TOOLU_CONFIG_DIR", temp.path().to_string_lossy())]),
    Some(Host::Claude),
  );
  let first = refresh(&roots, None).expect("first refresh");
  assert!(first.published);
  assert!(first.migrated);
  assert!(first.notice.is_some());
  assert_eq!(
    fs::read_to_string(temp.path().join("settings.json.bak")).expect("backup"),
    original
  );
  let second = refresh(&roots, None).expect("second refresh");
  assert!(!second.migrated);
  assert!(second.notice.is_none());
}

#[test]
fn installed_hook_publishes_a_plugin_owned_shim() {
  let temp = tempfile::tempdir().expect("temp directory");
  let plugin_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/statusline");
  let roots = Roots::new(
    Env::from_pairs([("TOOLU_CONFIG_DIR", temp.path().to_string_lossy())]),
    Some(Host::Opencode),
  );
  let done = refresh(&roots, Some(&plugin_root)).expect("installed refresh");
  assert!(done.published);
  assert!(!done.migrated);
  assert_eq!(
    fs::read_link(temp.path().join("statusline/statusline.sh")).expect("published link"),
    plugin_root.join("scripts/statusline.sh")
  );
}
