use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::register;

#[test]
fn register_writes_real_valid_manifests_twice() {
  let dir = tempfile::tempdir().expect("config root");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().display().to_string())]);
  for _ in 0..2 {
    let result = register(&env, Host::Claude);
    assert_eq!(result.exit.code(), 0);
  }
  let pre = dir
    .path()
    .join("toolu/pre-tools.d/ast-grep@toolu__search-nudge.json");
  let post = dir
    .path()
    .join("toolu/post-tools.d/ast-grep@toolu__byte-savings.json");
  assert!(pre.is_file() && post.is_file());
  let read = toolu_runtime::registry::manifest::read_manifest(
    &pre,
    toolu_runtime::registry::RegistryEvent::ToolPre,
  )
  .expect("valid manifest");
  assert_eq!(read.matcher, "Grep|Bash|Shell");
}

#[test]
fn register_replaces_stale_module_with_migration_hint_once() {
  let dir = tempfile::tempdir().expect("config root");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().display().to_string())]);
  let registry = dir.path().join("toolu/pre-tools.d");
  std::fs::create_dir_all(&registry).expect("registry");
  let old = registry.join("ast-grep@toolu__search-nudge.js");
  std::fs::write(&old, "old bundle").expect("old module");
  let first = register(&env, Host::Claude);
  assert_eq!(first.exit.code(), 0);
  assert!(!old.exists());
  assert!(
    first
      .stdout
      .is_some_and(|message| message.contains("toolu ast-grep search"))
  );
  assert!(register(&env, Host::Claude).stdout.is_none());
}
