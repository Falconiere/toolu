use std::path::Path;

use std::time::Duration;

use toolu_protocol::exit::Exit;

use crate::lima::Lima;
use crate::lima_stub::Stub;
use crate::state::State;
use crate::step::Step;

fn doc(step: Step) -> serde_json::Value {
  match step {
    Step::Report(report) => report.doc,
    Step::Launch(launch) => panic!("launched {launch:?}"),
  }
}

fn backend<'a>(stub: &Stub, context: &'a crate::context::Context) -> Lima<'a> {
  Lima {
    context,
    program: stub.program(),
    state: State::new(&context.env, "toolu-sandbox").waiting(Duration::from_millis(500)),
  }
}

#[test]
fn start_boots_a_stopped_vm_and_leaves_a_running_one() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  assert_eq!(lima.start(false).unwrap_err().exit, Exit::Unavailable);
  stub.list(Some(("Stopped", &[]))).unwrap();
  let started = doc(lima.start(false).unwrap());
  assert_eq!(
    (started["state"].as_str(), started["changed"].as_bool()),
    (Some("running"), Some(true))
  );
  stub
    .list(Some(("Running", &[stub.repo().as_path()])))
    .unwrap();
  let already = doc(lima.start(false).unwrap());
  assert_eq!(already["changed"], false);
  stub
    .list(Some(("Running", &[Path::new("/elsewhere")])))
    .unwrap();
  assert_eq!(lima.start(false).unwrap_err().exit, Exit::Unavailable);
  let restarted = doc(lima.start(true).unwrap());
  assert_eq!(restarted["changed"], true);
  let calls = stub.calls();
  assert!(
    calls.contains(&"--tty=false stop toolu-sandbox".to_owned()),
    "{calls:?}"
  );
}

#[test]
fn stop_is_refused_while_another_command_changes_the_vm() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  stub.list(Some(("Running", &[]))).unwrap();
  let held = lima.state.lock().unwrap();
  assert_eq!(lima.stop().unwrap_err().exit, Exit::TempFail);
  assert_eq!(stub.calls(), Vec::<String>::new());
  drop(held);
  assert_eq!(doc(lima.stop().unwrap())["changed"], true);
}

#[test]
fn stop_and_delete_act_only_on_a_vm_that_is_there() {
  let stub = Stub::new("2.2.1").unwrap();
  let context = stub.context();
  let lima = backend(&stub, &context);
  assert_eq!(doc(lima.stop().unwrap())["state"], "absent");
  assert_eq!(doc(lima.delete().unwrap())["changed"], false);
  stub.list(Some(("Stopped", &[]))).unwrap();
  assert_eq!(doc(lima.stop().unwrap())["changed"], false);
  stub.list(Some(("Running", &[]))).unwrap();
  let stopped = doc(lima.stop().unwrap());
  assert_eq!(
    (stopped["state"].as_str(), stopped["changed"].as_bool()),
    (Some("stopped"), Some(true))
  );
  lima.state.create().unwrap();
  crate::state::write(&lima.state.stamp(), "{}").unwrap();
  let deleted = doc(lima.delete().unwrap());
  assert_eq!(
    (deleted["state"].as_str(), deleted["changed"].as_bool()),
    (Some("absent"), Some(true))
  );
  assert!(!lima.state.stamp().exists());
  assert_eq!(
    stub.calls(),
    [
      "--tty=false stop toolu-sandbox",
      "--tty=false delete --force toolu-sandbox"
    ]
  );
  stub.fail("stop", 2).unwrap();
  let err = lima.stop().unwrap_err();
  assert_eq!(err.exit, Exit::Failure);
  assert!(
    err.message.ends_with("failed: stub stop failed"),
    "{}",
    err.message
  );
}
