//! The version-skew cases of #411 through the launcher (AC-5).

use std::path::Path;

use crate::sandbox::{Res, STARTUP, Sandbox, TOOLU, VERSION, copy_executable, system_message};

/// `VERSION` with its major moved by `major`, or else its minor by `minor`.
fn shifted(major: u64, minor: i64) -> Res<String> {
  let mut parts = VERSION.split('.').map(str::parse::<u64>);
  let (current_major, current_minor) = (
    parts.next().ok_or("no major")??,
    parts.next().ok_or("no minor")??,
  );
  if major > 0 {
    return Ok(format!("{}.0.0", current_major + major));
  }
  let moved = current_minor
    .checked_add_signed(minor)
    .ok_or("minor out of range")?;
  Ok(format!("{current_major}.{moved}.0"))
}

fn start(sandbox: &Sandbox) -> Res<String> {
  let run = sandbox.launch("SessionStart", "session-start", STARTUP, &[])?;
  if run.code != 0 {
    return Err(format!("SessionStart exited {}: {run:?}", run.code).into());
  }
  system_message(&run)
}

fn first_line(text: &str) -> &str {
  text.lines().next().unwrap_or_default()
}

#[test]
fn the_same_version_runs_without_advice() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  assert!(
    start(&sandbox)
      .unwrap()
      .starts_with("toolu runtime: native")
  );
}

#[test]
fn an_older_binary_advises_the_installer_and_still_enforces() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let plugin = shifted(0, 1).unwrap();
  sandbox.manifest(&plugin, "1").unwrap();
  assert_eq!(
    first_line(&start(&sandbox).unwrap()),
    format!(
      "toolu {VERSION} is older than the toolu plugin {plugin}; \
       upgrade it: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"
    )
  );
  let pre = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(pre.code, 0, "{pre:?}");
}

#[test]
fn a_binary_in_a_cellar_advises_brew_upgrade() {
  let sandbox = Sandbox::new().unwrap();
  let cellar = sandbox.home.join(format!("Cellar/toolu/{VERSION}/bin"));
  std::fs::create_dir_all(&cellar).unwrap();
  copy_executable(Path::new(TOOLU), &cellar.join("toolu")).unwrap();
  std::os::unix::fs::symlink(cellar.join("toolu"), sandbox.local_bin()).unwrap();
  sandbox.manifest(&shifted(0, 1).unwrap(), "1").unwrap();
  let message = start(&sandbox).unwrap();
  assert!(
    first_line(&message).ends_with("upgrade it: brew upgrade toolu"),
    "{message}"
  );
  assert!(message.contains(&format!("Cellar/toolu/{VERSION}/bin/toolu")));
}

#[test]
fn a_newer_binary_advises_a_plugin_update() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  sandbox.manifest(&shifted(0, -1).unwrap(), "1").unwrap();
  let message = start(&sandbox).unwrap();
  assert!(message.contains(&format!("toolu {VERSION} is newer than the toolu plugin")));
  assert!(message.contains("update the plugins from your host's marketplace"));
}

#[test]
fn a_major_skew_with_the_same_protocol_only_advises() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  sandbox.manifest(&shifted(1, 0).unwrap(), "1").unwrap();
  assert!(
    start(&sandbox)
      .unwrap()
      .contains("is older than the toolu plugin")
  );
  let pre = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(pre.code, 0, "{pre:?}");
}

#[test]
fn a_higher_plugin_protocol_blocks_with_the_upgrade_command() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  sandbox.manifest(VERSION, "2").unwrap();
  let pre = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &[])
    .unwrap();
  assert_eq!(pre.code, 2, "{pre:?}");
  assert!(
    pre
      .stderr
      .starts_with("blocked: toolu plugin: hook protocol 2 needs a newer toolu")
  );
  assert!(pre.stderr.contains("upgrade it: curl -fsSL"));
  let message = start(&sandbox).unwrap();
  assert!(message.starts_with("toolu plugin: hook protocol 2 needs a newer toolu"));
  assert!(!message.contains("toolu runtime"));
}

#[test]
fn a_bad_protocol_value_is_a_bad_manifest() {
  for value in ["0", "\"1\""] {
    let sandbox = Sandbox::new().unwrap();
    sandbox.install().unwrap();
    sandbox.manifest(VERSION, value).unwrap();
    let pre = sandbox
      .launch("PreToolUse", "session-start", STARTUP, &[])
      .unwrap();
    assert_eq!(pre.code, 2, "{value}: {pre:?}");
    assert!(
      pre.stderr.contains("cannot read hookProtocol from"),
      "{value}"
    );
    assert!(pre.stderr.contains("update the plugins"), "{value}");
  }
}
