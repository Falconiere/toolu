use std::fs;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::setup;

#[test]
fn existing_custom_setting_is_refused_without_a_backup() {
  let temp = tempfile::tempdir().expect("temp directory");
  let path = temp.path().join("settings.json");
  let original = r#"{"statusLine":{"type":"command","command":"my-custom-bar"}}"#;
  fs::write(&path, original).expect("settings");
  let roots = Roots::new(
    Env::from_pairs([("TOOLU_CONFIG_DIR", temp.path().to_string_lossy())]),
    Some(Host::Claude),
  );
  let result = setup(&roots, false);
  assert_eq!(result.status, "REFUSED");
  assert_eq!(result.exit.code(), 2);
  assert_eq!(fs::read_to_string(path).expect("unchanged"), original);
  assert!(!temp.path().join("settings.json.bak").exists());
}
