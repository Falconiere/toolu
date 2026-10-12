//! Replacing a running engine: at once without one, and only after a
//! running effect returns with one.

use crate::disk::read_value;
use crate::paths::Paths;
use crate::server::Fault;

#[test]
fn replace_returns_at_once_without_a_live_engine() {
  let tmp = tempfile::tempdir().expect("temp");
  let started = std::time::Instant::now();
  assert!(super::replace_engine(
    &Paths::at(tmp.path()),
    std::time::Duration::from_secs(5)
  ));
  assert!(started.elapsed() < std::time::Duration::from_secs(1));
  assert_eq!(
    super::DRAINING,
    "engine draining; the new engine starts when it exits"
  );
}

#[test]
fn replace_waits_for_a_running_effect_and_the_old_engine_exits_after_it() {
  use std::os::unix::fs::PermissionsExt;
  let fix = crate::source_fix::fixture(true).expect("fixture");
  crate::source_fix::in_repo(&fix).expect("repo");
  let (scripts, release) = (
    fix.tmp.path().join("scripts"),
    fix.tmp.path().join("release"),
  );
  std::fs::create_dir_all(&scripts).expect("scripts");
  let merge = scripts.join("merge");
  std::fs::write(
    &merge,
    format!(
      "#!/bin/sh\nwhile [ ! -f '{}' ]; do sleep 0.05; done\necho '{{\"outcome\":\"applied\"}}'\n",
      release.display()
    ),
  )
  .expect("script");
  std::fs::set_permissions(&merge, std::fs::Permissions::from_mode(0o755)).expect("chmod");
  let paths = fix.engine_paths;
  let serving = paths.clone();
  let engine = std::thread::spawn(move || {
    let options = crate::socket::ServeOptions {
      http: None,
      tick: &(),
      attention: crate::notify::AttentionOptions::default(),
    };
    crate::socket::serve_with_http(
      serving,
      Some(scripts),
      Fault::None,
      crate::PROTOCOL,
      options,
    )
  });
  let epic = read_value(&paths.registry()).expect("registry")["epics"][0]["state_dir"]
    .as_str()
    .map(str::to_owned)
    .expect("epic");
  let ready = serde_json::json!({"op": "report", "key": "a", "epic": "one", "state_dir": epic, "phase": "ready", "pr": 412, "note": ""});
  crate::client::exchange_retry(&paths, crate::PROTOCOL, &ready).expect("report");
  std::thread::sleep(std::time::Duration::from_millis(300));
  assert!(
    !super::replace_engine(&paths, std::time::Duration::from_millis(200)),
    "the effect still runs"
  );
  assert!(crate::lock::live(&paths.lock()));
  std::fs::write(&release, "").expect("release");
  engine.join().expect("engine thread").expect("serve");
  assert!(!crate::lock::live(&paths.lock()));
}
