use std::fs;

use toolu_runtime::env::Env;

use super::session_start;

#[test]
fn session_start_publishes_shim_but_preserves_a_user_file() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("TOOLU_HOST_OVERRIDE", "codex".to_owned()),
    ("TOOLU_CONFIG_DIR", config.display().to_string()),
  ]);
  let plugin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu-review");
  let result = session_start(&env, &plugin);
  assert!(result.stderr.is_none(), "{result:?}");
  let shim = config.join("toolu-review/write-state.sh");
  assert!(
    fs::symlink_metadata(&shim)
      .unwrap()
      .file_type()
      .is_symlink()
  );
  fs::remove_file(&shim).unwrap();
  fs::write(&shim, "user override\n").unwrap();
  let result = session_start(&env, &plugin);
  assert!(result.stderr.is_none(), "{result:?}");
  assert_eq!(fs::read_to_string(shim).unwrap(), "user override\n");
}
