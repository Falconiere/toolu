use std::path::Path;
use std::thread;
use std::time::Duration;

use clap::{Arg, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{
  ack, answer, ask, epic_for, install_unit, job, located, pause, report, service, status, wait,
  wait_default,
};
use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::serve;

#[test]
fn a_status_path_yields_the_issue_key_and_state_dir() {
  let (key, dir) = located(Path::new("/epics/one/status/a.json")).expect("path");
  assert_eq!(key, "a");
  assert_eq!(dir, "/epics/one");
  assert!(located(Path::new("a.json")).is_err());
  assert!(located(Path::new("/")).is_err());
}

#[test]
fn wait_defaults_follow_the_host() {
  assert_eq!(wait_default(None), 2700);
  assert_eq!(wait_default(Some(Host::Claude)), 2700);
  assert_eq!(wait_default(Some(Host::Codex)), 480);
  assert_eq!(wait_default(Some(Host::Cursor)), 480);
  assert_eq!(wait_default(Some(Host::Opencode)), 480);
  assert_eq!(wait_default(Some(Host::Hermes)), 480);
}

#[test]
fn epic_for_reads_the_registry_row() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  assert_eq!(epic_for(&paths, "/epic"), "");
  crate::disk::write_value(&paths.registry(), &json!({"version": 1})).expect("registry");
  assert_eq!(epic_for(&paths, "/epic"), "");
  crate::disk::write_value(
    &paths.registry(),
    &json!({"epics": [{"state_dir": "/epic"}, {"key": "one", "state_dir": "/epic"}]}),
  )
  .expect("rows");
  assert_eq!(epic_for(&paths, "/epic"), "");
  crate::disk::write_value(
    &paths.registry(),
    &json!({"epics": [{"key": "one", "state_dir": "/epic"}]}),
  )
  .expect("row");
  assert_eq!(epic_for(&paths, "/epic"), "one");
  assert_eq!(epic_for(&paths, "/other"), "");
  std::fs::remove_file(paths.registry()).expect("remove");
  std::fs::create_dir(paths.registry()).expect("dir");
  assert_eq!(epic_for(&paths, "/epic"), "");
}

#[test]
fn report_job_and_install_fail_closed() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_paths, env) = root(tmp.path());
  let missing = report(
    &Command::new("report")
      .arg(Arg::new("phase"))
      .arg(Arg::new("status-file"))
      .get_matches_from(["report"]),
    &env,
  );
  assert!(missing.stderr.expect("err").contains("phase is required"));
  let no_file = Command::new("report")
    .arg(Arg::new("phase"))
    .arg(Arg::new("status-file"))
    .get_matches_from(["report", "execution"]);
  assert!(
    report(&no_file, &env)
      .stderr
      .expect("err")
      .contains("status-file is required")
  );
  let bad = Command::new("report")
    .arg(Arg::new("phase"))
    .arg(Arg::new("status-file").long("status-file"))
    .get_matches_from(["report", "execution", "--status-file", "a.json"]);
  assert!(report(&bad, &env).stderr.expect("err").contains("status"));
  let outcome = job(
    &Command::new("job")
      .arg(Arg::new("argv").num_args(0..))
      .get_matches_from(["job"]),
  );
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(
    outcome
      .stderr
      .expect("err")
      .contains("a command is required")
  );
  let home = tmp.path().join("home-file");
  std::fs::write(&home, "x").expect("home");
  let err = install_unit(&Paths::at(tmp.path()), &home, Path::new("/bin/toolu")).expect_err("home");
  assert!(err.contains("could not write"));
  let file = tmp.path().join("not-a-dir");
  std::fs::write(&file, "x").expect("file");
  let err = install_unit(&Paths::at(&file), tmp.path(), Path::new("/bin/toolu")).expect_err("root");
  assert!(err.contains("could not write"));
}

#[test]
fn service_writes_the_unit_for_its_home() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, env) = root(tmp.path());
  let outcome = service(&env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let body = std::fs::read_to_string(paths.service()).expect("unit");
  assert!(body.contains("WantedBy=default.target"));
  assert!(body.contains(&paths.root.display().to_string()));
}

#[test]
fn wait_returns_waiting_when_the_engine_is_down() {
  let tmp = tempfile::tempdir().expect("temp");
  let (_paths, env) = root(tmp.path());
  let ctx = Ctx {
    host: Some(Host::Codex),
    ..Ctx::default()
  };
  let outcome = wait(&ctx, &parse(&["epic", "wait"]), &env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert!(
    outcome
      .stdout
      .expect("json")
      .contains("\"state\":\"waiting\"")
  );
}

#[test]
fn status_and_pause_fail_when_the_socket_never_opens() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, env) = root(tmp.path());
  crate::disk::write_value(
    &paths.registry(),
    &json!({"epics": [{"key": "one", "state_dir": "/epic"}]}),
  )
  .expect("registry");
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  let outcome = status(&parse(&["epic", "status"]), &env);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(outcome.stderr.expect("err").contains("toolu epic status"));
  std::fs::remove_file(paths.lock()).expect("unlock");
  std::fs::remove_file(paths.registry()).expect("clear");
  let outcome = pause(&parse(&["epic", "pause"]), &env, true);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(outcome.stderr.expect("err").contains("toolu epic pause"));
}

#[test]
fn verbs_round_trip_on_a_local_engine() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, env) = root(tmp.path());
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  crate::disk::write_value(
    &paths.registry(),
    &json!({"version": 1, "epics": [{"key": "one", "state_dir": epic.display().to_string()}]}),
  )
  .expect("registry");
  let background = paths.clone();
  let handle = thread::spawn(move || serve(background, None, Fault::None, PROTOCOL));
  wait_live(&paths);
  assert_status_pause_and_report(&env, &epic);
  assert_wait_ack_and_answer(&env);
  exchange_retry(&paths, PROTOCOL, &json!({"op": "stop"})).expect("stop");
  handle.join().expect("engine").expect("serve");
}

fn assert_status_pause_and_report(env: &Env, epic: &Path) {
  let listed = status(&parse(&["epic", "status", "one"]), env);
  assert_eq!(listed.exit, Exit::Success, "{listed:?}");
  assert!(
    listed
      .stdout
      .expect("status")
      .contains("\"engine\":\"running\"")
  );
  assert_eq!(
    pause(&parse(&["epic", "pause", "one"]), env, true).exit,
    Exit::Success
  );
  assert_eq!(
    pause(&parse(&["epic", "resume"]), env, false).exit,
    Exit::Success
  );
  let file = epic.join("status").join("a.json");
  let reported = report(
    &parse(&[
      "epic",
      "report",
      "execution",
      "--status-file",
      &file.display().to_string(),
      "--pr",
      "7",
      "--note",
      "hi",
    ]),
    env,
  );
  assert_eq!(reported.exit, Exit::Success, "{reported:?}");
}

fn assert_wait_ack_and_answer(env: &Env) {
  let waiting = wait(
    &Ctx::default(),
    &parse(&["epic", "wait", "--max-seconds", "0"]),
    env,
  );
  assert_eq!(waiting.exit, Exit::Success, "{waiting:?}");
  assert_eq!(ack(&parse(&["epic", "ack", "a"]), env).exit, Exit::Success);
  assert_eq!(
    answer(&parse(&["epic", "answer", "a", "yes"]), env).exit,
    Exit::Success
  );
  assert_eq!(
    ask(env, "status", &json!({"op": "status"})).exit,
    Exit::Success
  );
}

pub(super) fn root(tmp: &Path) -> (Paths, Env) {
  let resources = tmp.join("resources");
  let home = tmp.join("home");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::create_dir_all(&home).expect("home");
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_RESOURCE_HOME", resources.display().to_string()),
  ]);
  (Paths::from_env(&env), env)
}

pub(super) fn parse(argv: &[&str]) -> clap::ArgMatches {
  let matches = crate::verbs::command()
    .try_get_matches_from(argv)
    .expect("parse");
  matches.subcommand().expect("subcommand").1.clone()
}

pub(super) fn wait_live(paths: &Paths) {
  for _ in 0..100 {
    if crate::lock::live(&paths.lock()) {
      return;
    }
    thread::sleep(Duration::from_millis(20));
  }
  panic!("engine did not lock");
}
