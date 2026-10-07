use std::path::Path;

use super::super::checks::Status;
use super::check;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn write(path: &Path, body: &str) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}

fn layout(home: &Path) {
  let dir = home.join(".claude/toolu/pre-tools.d");
  write(&dir.join("x@vendor__mod.js"), "");
  write(&dir.join("x@vendor__mod.sh"), "");
  write(&dir.join("loose.js"), "");
  write(
    &dir.join("jev@toolu__rule.json"),
    r#"{"version":1,"spec":"jev@toolu","name":"rule","event":"tool/pre","matcher":"*"}"#,
  );
  write(
    &dir.join("x@vendor__bad.json"),
    r#"{"version":2,"spec":"x@vendor","name":"bad","event":"tool/pre","matcher":"*"}"#,
  );
  write(
    &home.join(".claude/plugins/installed_plugins.json"),
    r#"{"plugins":{}}"#,
  );
}

#[test]
fn javascript_and_orphans_warn_a_bad_version_fails_and_shell_is_silent() {
  let home = tempfile::tempdir().unwrap();
  layout(home.path());
  let roots = Roots::new(
    Env::from_pairs([
      ("HOME", home.path().to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.path().to_str().unwrap()),
    ]),
    Some(Host::Claude),
  );
  let check = check(&roots);
  assert_eq!(check.status, Status::Fail, "{}", check.summary);
  assert!(
    check
      .summary
      .contains("x@vendor__mod.js runs through the Bun bridge until #440"),
    "{}",
    check.summary
  );
  assert!(
    check
      .summary
      .contains("jev@toolu__rule.json is an orphaned manifest"),
    "{}",
    check.summary
  );
  assert!(
    check.summary.contains("unsupported version 2"),
    "{}",
    check.summary
  );
  assert!(
    check.summary.contains("loose.js is unnamespaced"),
    "{}",
    check.summary
  );
  assert!(
    !check.summary.contains("x@vendor__mod.sh"),
    "{}",
    check.summary
  );
}
