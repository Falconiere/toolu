use std::fs;
use std::os::unix::fs::symlink;

use toolu_runtime::env::Env;

use super::session_start;

#[test]
fn session_start_retires_the_old_link_but_preserves_a_user_file() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("config");
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("TOOLU_HOST_OVERRIDE", "codex".to_owned()),
    ("TOOLU_CONFIG_DIR", config.display().to_string()),
  ]);
  let plugin = dir.path().join("cache/toolu-review/7.11.0");
  let shim = config.join("toolu-review/write-state.sh");
  fs::create_dir_all(shim.parent().unwrap()).unwrap();
  symlink(plugin.join("scripts/write-state.sh"), &shim).unwrap();
  let result = session_start(&env, &plugin);
  assert_eq!(
    (result.stdout.as_deref(), result.stderr.as_deref()),
    (None, None)
  );
  assert!(fs::symlink_metadata(&shim).is_err(), "the old link is gone");
  fs::write(&shim, "user override\n").unwrap();
  let result = session_start(&env, &plugin);
  assert!(result.stderr.is_none(), "{result:?}");
  assert_eq!(fs::read_to_string(shim).unwrap(), "user override\n");
}
