use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::{
  CodexPluginSnapshot, Installed, SnapshotResult, SnapshotStatus, canonical,
  codex_plugin_installed, codex_plugin_snapshot_path, snapshot_codex_plugins,
};
use crate::env::Env;
use crate::host::roots::Roots;

fn ready(plugins: &[&str]) -> CodexPluginSnapshot {
  let plugins = plugins.iter().map(|id| (*id).to_owned()).collect();
  CodexPluginSnapshot {
    version: 1,
    status: SnapshotStatus::Ready,
    plugins,
  }
}

fn indeterminate() -> CodexPluginSnapshot {
  CodexPluginSnapshot {
    version: 1,
    status: SnapshotStatus::Indeterminate,
    plugins: Vec::new(),
  }
}

#[test]
fn installed_and_enabled_ids_are_kept_once_in_utf16_order() {
  let listing = r#"{"installed":[
    {"pluginId":"z@m"},
    {"pluginId":"a@m","installed":true,"enabled":true},
    {"pluginId":"off@m","enabled":false},
    {"pluginId":"gone@m","installed":"yes"},
    {"name":"named","marketplaceName":"m"},
    {"pluginId":null,"name":"fallback","marketplaceName":"m"},
    {"pluginId":"","name":"x","marketplaceName":"m"},
    {"pluginId":5},
    {"name":"no-market"},
    {"pluginId":"z@m"},
    {"pluginId":"｡"},
    {"pluginId":"😀"}
  ],"other":1}"#;
  let expected = ready(&[
    "a@m",
    "fallback@m",
    "named@m",
    "z@m",
    "\u{1f600}",
    "\u{ff61}",
  ]);
  assert_eq!(canonical(Some(listing)), expected);
}

#[test]
fn a_listing_jq_would_reject_is_indeterminate() {
  for listing in [
    None,
    Some("not json"),
    Some("[]"),
    Some(r#"{"installed":{"a":{}}}"#),
    Some(r#"{"installed":[null]}"#),
    Some(r#"{"installed":[[]]}"#),
  ] {
    assert_eq!(canonical(listing), indeterminate(), "{listing:?}");
  }
  assert_eq!(canonical(Some(r#"{"installed":[]}"#)), ready(&[]));
}

#[test]
fn toolu_config_dir_wins_on_codex() {
  let base = [
    ("HOME", "/home/u"),
    ("CODEX_HOME", "/cx"),
    ("PLUGIN_ROOT", "/p"),
  ];
  let codex = Roots::new(Env::from_pairs(base), None);
  assert_eq!(
    codex_plugin_snapshot_path(&codex),
    PathBuf::from("/cx/toolu/codex-plugins.json")
  );
  let toolu = Roots::new(Env::from_pairs(base).with("TOOLU_CONFIG_DIR", "/t"), None);
  assert_eq!(
    codex_plugin_snapshot_path(&toolu),
    PathBuf::from("/t/toolu/codex-plugins.json")
  );
  let explicit = Env::from_pairs(base).with("TOOLU_CODEX_PLUGIN_SNAPSHOT", "/s.json");
  assert_eq!(
    codex_plugin_snapshot_path(&Roots::new(explicit, None)),
    PathBuf::from("/s.json")
  );
}

fn codex_env(dir: &Path, listing: &str) -> Env {
  let bin = dir.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let script = bin.join("codex");
  std::fs::write(
    &script,
    format!("#!/bin/sh\ncat <<'JSON'\n{listing}\nJSON\n"),
  )
  .unwrap();
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
  let config = dir.join("config").display().to_string();
  Env::from_pairs([
    ("PATH", path.as_str()),
    ("TOOLU_CONFIG_DIR", config.as_str()),
  ])
}

#[test]
fn codex_writes_the_snapshot_bytes_and_reads_them_back() {
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(
    codex_env(dir.path(), r#"{"installed":[{"pluginId":"jev@toolu"}]}"#),
    Some(Host::Codex),
  );
  let result: SnapshotResult = snapshot_codex_plugins(&roots).unwrap();
  assert!(result.written);
  assert_eq!(result.snapshot, ready(&["jev@toolu"]));
  let bytes = std::fs::read_to_string(&result.path).unwrap();
  assert_eq!(
    bytes,
    "{\"version\":1,\"status\":\"ready\",\"plugins\":[\"jev@toolu\"]}\n"
  );
  assert_eq!(
    codex_plugin_installed("jev@toolu", &roots),
    Installed::Installed
  );
  assert_eq!(
    codex_plugin_installed("toolu@toolu", &roots),
    Installed::Absent
  );
  assert_eq!(codex_plugin_installed("", &roots), Installed::Absent);
}

#[test]
fn an_indeterminate_or_missing_snapshot_reads_as_unknown() {
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(codex_env(dir.path(), "garbage"), Some(Host::Codex));
  assert_eq!(
    codex_plugin_installed("jev@toolu", &roots),
    Installed::Unknown
  );
  let result = snapshot_codex_plugins(&roots).unwrap();
  assert_eq!(result.snapshot, indeterminate());
  assert_eq!(
    codex_plugin_installed("jev@toolu", &roots),
    Installed::Unknown
  );
  std::fs::write(
    &result.path,
    r#"{"version":2,"status":"ready","plugins":["jev@toolu"]}"#,
  )
  .unwrap();
  assert_eq!(
    codex_plugin_installed("jev@toolu", &roots),
    Installed::Unknown
  );
  std::fs::write(
    &result.path,
    r#"{"version":1,"status":"ready","plugins":"jev@toolu"}"#,
  )
  .unwrap();
  assert_eq!(
    codex_plugin_installed("jev@toolu", &roots),
    Installed::Unknown
  );
}

#[test]
fn other_hosts_write_nothing_and_an_unwritable_path_is_reported() {
  let dir = tempfile::tempdir().unwrap();
  let env = codex_env(dir.path(), r#"{"installed":[]}"#);
  assert_eq!(
    snapshot_codex_plugins(&Roots::new(env.clone(), Some(Host::Claude))),
    None
  );
  std::fs::write(dir.path().join("config"), "a file where the directory goes").unwrap();
  let result = snapshot_codex_plugins(&Roots::new(env, Some(Host::Codex))).unwrap();
  assert!(!result.written);
}
