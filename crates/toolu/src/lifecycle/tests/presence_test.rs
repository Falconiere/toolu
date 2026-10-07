use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::snapshot::Installed;

use super::{install_command, plugin_active, presence};

#[test]
fn non_codex_hosts_read_claudes_install_record() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("cfg");
  let plugins = root.join("plugins");
  std::fs::create_dir_all(&plugins).unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", root.to_str().unwrap()),
  ]);
  assert_eq!(
    presence("ast-grep@toolu", &env, Host::Cursor),
    Installed::Unknown
  );
  assert!(plugin_active("ast-grep@toolu", &env, Host::Cursor));
  std::fs::write(plugins.join("installed_plugins.json"), r#"{"plugins":{}}"#).unwrap();
  assert_eq!(
    presence("ast-grep@toolu", &env, Host::Hermes),
    Installed::Absent
  );
  assert!(!plugin_active("ast-grep@toolu", &env, Host::Opencode));
  std::fs::write(
    plugins.join("installed_plugins.json"),
    r#"{"plugins":{"ast-grep@toolu":{}}}"#,
  )
  .unwrap();
  assert_eq!(
    presence("ast-grep@toolu", &env, Host::Claude),
    Installed::Installed
  );
  assert_eq!(
    install_command("ast-grep@toolu", Host::Claude),
    "/plugin install ast-grep@toolu"
  );
  assert_eq!(
    install_command("ast-grep@toolu", Host::Codex),
    "codex plugin add ast-grep@toolu"
  );
}
