use std::path::Path;

use super::super::checks::Status;
use super::super::inventory::collect;
use super::check;
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn install(home: &Path, version: &str, protocol: u32) -> Roots {
  let root = home.join("toolu");
  let manifest = root.join(".claude-plugin/plugin.json");
  std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
  std::fs::write(
    &manifest,
    format!(r#"{{"name":"toolu","version":"{version}","hookProtocol":{protocol}}}"#),
  )
  .unwrap();
  let record = home.join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(
    &record,
    format!(
      r#"{{"plugins":{{"toolu@toolu":[{{"scope":"user","installPath":"{}"}}]}}}}"#,
      root.display()
    ),
  )
  .unwrap();
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.to_str().unwrap()),
    ]),
    Some(Host::Claude),
  )
}

#[test]
fn a_different_version_warns_and_a_different_protocol_fails() {
  let home = tempfile::tempdir().unwrap();
  let roots = install(home.path(), "0.0.1", HOOK_PROTOCOL);
  let warned = check(&collect(&roots, home.path()));
  assert_eq!(warned.status, Status::Warn, "{}", warned.summary);
  assert!(warned.summary.contains("0.0.1"), "{}", warned.summary);

  let roots = install(home.path(), "7.11.0", 99);
  let failed = check(&collect(&roots, home.path()));
  assert_eq!(failed.status, Status::Fail, "{}", failed.summary);
  assert!(
    failed.summary.contains("hook protocol 99"),
    "{}",
    failed.summary
  );
}

#[test]
fn no_plugins_make_skew_ok() {
  let home = tempfile::tempdir().unwrap();
  let record = home.path().join(".claude/plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(&record, r#"{"plugins":{}}"#).unwrap();
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", home.path().to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.path().to_str().unwrap()),
    ]),
    Some(Host::Claude),
  );
  let check = check(&collect(&roots, home.path()));
  assert_eq!(check.status, Status::Ok);
  assert_eq!(check.summary, "no plugins to compare");
}
