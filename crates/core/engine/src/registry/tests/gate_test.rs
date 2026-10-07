use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::Installed;

use super::{plugin_active, plugin_presence};

fn record(path: &Path, text: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, text).unwrap();
}

const INSTALLED: &str = r#"{"version":2,"plugins":{"x@t":[{"scope":"user"}]}}"#;

fn claude(env: Env) -> Roots {
  Roots::new(env, Some(Host::Claude))
}

#[test]
fn claude_reads_its_install_record_from_home() {
  let home = tempfile::tempdir().unwrap();
  let h = home.path().to_str().unwrap();
  record(
    &home.path().join(".claude/plugins/installed_plugins.json"),
    INSTALLED,
  );
  let roots = claude(Env::from_pairs([("HOME", h)]));
  assert_eq!(plugin_presence("x@t", &roots), Installed::Installed);
  assert_eq!(plugin_presence("y@t", &roots), Installed::Absent);
  assert!(plugin_active("x@t", &roots) && !plugin_active("y@t", &roots));
  assert_eq!(plugin_presence("", &roots), Installed::Absent);
}

#[test]
fn the_record_path_follows_the_variable_chain() {
  let dir = tempfile::tempdir().unwrap();
  let d = dir.path();
  let explicit = d.join("explicit.json");
  record(&explicit, INSTALLED);
  record(&d.join("toolu/plugins/installed_plugins.json"), INSTALLED);
  record(&d.join("claude/plugins/installed_plugins.json"), INSTALLED);
  let path = |p: &Path| p.to_str().unwrap().to_owned();
  for (key, value) in [
    ("CLAUDE_PLUGINS_REGISTRY", path(&explicit)),
    ("TOOLU_CONFIG_DIR", path(&d.join("toolu"))),
    ("CLAUDE_CONFIG_DIR", path(&d.join("claude"))),
  ] {
    let roots = claude(Env::from_pairs([
      ("HOME", "/nonexistent"),
      (key, value.as_str()),
    ]));
    assert_eq!(
      plugin_presence("x@t", &roots),
      Installed::Installed,
      "{key}"
    );
  }
}

#[test]
fn an_unreadable_record_fails_open() {
  let home = tempfile::tempdir().unwrap();
  let h = home.path().to_str().unwrap();
  let file = home.path().join(".claude/plugins/installed_plugins.json");
  let roots = claude(Env::from_pairs([("HOME", h)]));
  assert_eq!(
    plugin_presence("x@t", &roots),
    Installed::Unknown,
    "absent record"
  );
  for text in ["not json", r#"{"plugins":[]}"#, r#"{"version":2}"#, "[]"] {
    record(&file, text);
    assert_eq!(plugin_presence("x@t", &roots), Installed::Unknown, "{text}");
    assert!(plugin_active("x@t", &roots));
  }
  std::fs::remove_file(&file).unwrap();
  std::fs::create_dir(&file).unwrap();
  assert_eq!(
    plugin_presence("x@t", &roots),
    Installed::Unknown,
    "a directory"
  );
}

#[test]
fn codex_reads_its_snapshot_and_other_hosts_are_unknown() {
  let codex = tempfile::tempdir().unwrap();
  let c = codex.path().to_str().unwrap();
  record(
    &codex.path().join("toolu/codex-plugins.json"),
    r#"{"version":1,"status":"ready","plugins":["x@t"]}"#,
  );
  let roots = Roots::new(Env::from_pairs([("CODEX_HOME", c)]), Some(Host::Codex));
  assert_eq!(plugin_presence("x@t", &roots), Installed::Installed);
  assert_eq!(plugin_presence("y@t", &roots), Installed::Absent);
  for host in [Host::Cursor, Host::Hermes, Host::Opencode] {
    let roots = Roots::new(Env::from_pairs([("HOME", c)]), Some(host));
    assert_eq!(
      plugin_presence("y@t", &roots),
      Installed::Unknown,
      "{host:?}"
    );
  }
}
