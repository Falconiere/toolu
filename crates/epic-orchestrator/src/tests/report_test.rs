use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::report;
use super::tests::{parse, root, wait_live};
use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::server::Fault;
use crate::socket::serve;

#[test]
fn report_spool_order() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, env) = root(tmp.path());
  let file = tmp.path().join("epic").join("status").join("a.json");
  let path = file.display().to_string();
  assert_eq!(send_phase(&env, "brainstorm", &path).exit, Exit::Success);
  assert_eq!(send_phase(&env, "spec", &path).exit, Exit::Success);
  let background = paths.clone();
  let handle = thread::spawn(move || serve(background, None, Fault::None, PROTOCOL, &()));
  wait_live(&paths);
  let rows = wait_phases(&paths);
  let phases: Vec<_> = rows
    .iter()
    .filter(|row| row.name == "phase")
    .map(|row| row.note.as_str())
    .collect();
  assert_eq!(phases, ["brainstorm", "spec"]);
  assert_eq!(history_len(&file), 2);
  let token = rows
    .iter()
    .find(|row| row.name == "spool")
    .expect("spool")
    .token
    .clone();
  exchange_retry(
    &paths,
    PROTOCOL,
    &json!({"op": "report", "token": token, "key": "a", "phase": "brainstorm", "note": ""}),
  )
  .expect("replay");
  assert_eq!(history_len(&file), 2);
  exchange_retry(&paths, PROTOCOL, &json!({"op": "stop"})).expect("stop");
  handle.join().expect("engine").expect("serve");
}

#[test]
fn report_ready_merges() {
  let fix = crate::source_fix::fixture(true).expect("fixture");
  let mut engine = crate::server::Engine::open(fix.engine_paths, None, Fault::None).expect("open");
  let dir = engine.world.issues["a"].state_dir.clone();
  engine
    .report(&crate::model::Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir: dir,
      phase: "ready".to_owned(),
      pr: Some(412),
      note: String::new(),
    })
    .expect("report");
  let pending = engine.world.issues["a"].pending.clone().expect("merge");
  assert_eq!(pending.action, crate::model::Action::Merge);
  assert_eq!(engine.world.issues["a"].pr, Some(412));
}

fn wait_phases(paths: &crate::paths::Paths) -> Vec<crate::journal::Record> {
  for _ in 0..100 {
    let rows =
      crate::journal::tail(&paths.journal_dir(), std::time::SystemTime::now()).unwrap_or_default();
    let ready = rows.iter().filter(|row| row.name == "phase").count() >= 2;
    if ready {
      return rows;
    }
    thread::sleep(Duration::from_millis(20));
  }
  crate::journal::tail(&paths.journal_dir(), std::time::SystemTime::now()).expect("journal")
}

fn send_phase(env: &Env, phase: &str, file: &str) -> toolu_runtime::cli::Outcome {
  report(
    &parse(&["epic", "report", phase, "--status-file", file]),
    env,
  )
}

fn history_len(file: &Path) -> usize {
  crate::disk::read_value(file)
    .expect("status")
    .get("history")
    .and_then(|value| value.as_array())
    .map_or(0, Vec::len)
}
