use std::path::PathBuf;
use std::time::{Duration, Instant};

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{State, write};

#[test]
fn the_files_live_under_the_xdg_state_home_or_local_state() {
  let xdg = State::new(
    &Env::from_pairs([("XDG_STATE_HOME", "/s"), ("HOME", "/h")]),
    "vm",
  );
  assert_eq!(xdg.yaml(), PathBuf::from("/s/toolu/sandbox/vm.yaml"));
  assert_eq!(
    xdg.log("setup"),
    PathBuf::from("/s/toolu/sandbox/vm-setup.log")
  );
  assert_eq!(xdg.stamp(), PathBuf::from("/s/toolu/sandbox/vm.json"));
  let home = State::new(&Env::from_pairs([("HOME", "/h")]), "vm");
  assert_eq!(
    home.yaml(),
    PathBuf::from("/h/.local/state/toolu/sandbox/vm.yaml")
  );
}

#[test]
fn the_lock_is_exclusive_and_released_on_drop() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
  let fast = State::new(&env, "vm").waiting(Duration::ZERO);
  let patient = State::new(&env, "vm");
  let held = fast.lock().unwrap();
  let busy = fast.lock().unwrap_err();
  assert_eq!(busy.exit, Exit::TempFail);
  assert!(
    busy
      .message
      .starts_with("another toolu sandbox command is changing vm")
  );
  drop(held);
  assert!(patient.lock().is_ok());
}

#[test]
fn a_lock_waits_for_a_holder_that_lets_go_and_gives_up_after_its_wait() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
  let state = State::new(&env, "vm");
  let held = state.lock().unwrap();
  let holder = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(300));
    drop(held);
  });
  let started = Instant::now();
  assert!(
    state
      .clone()
      .waiting(Duration::from_secs(30))
      .lock()
      .is_ok()
  );
  assert!(started.elapsed() >= Duration::from_millis(250));
  holder.join().unwrap();
  let held = state.lock().unwrap();
  let started = Instant::now();
  let busy = state.waiting(Duration::from_millis(200)).lock();
  assert_eq!(busy.unwrap_err().exit, Exit::TempFail);
  assert!(started.elapsed() >= Duration::from_millis(200));
  drop(held);
}

#[test]
fn written_files_are_forgotten_and_failures_name_the_path() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("XDG_STATE_HOME", dir.path().to_str().unwrap())]);
  let state = State::new(&env, "vm");
  state.create().unwrap();
  write(&state.yaml(), "cpus: 2\n").unwrap();
  write(&state.stamp(), "{}").unwrap();
  assert_eq!(std::fs::read_to_string(state.yaml()).unwrap(), "cpus: 2\n");
  state.forget();
  assert!(!state.yaml().exists() && !state.stamp().exists());
  let blocked = dir.path().join("file");
  std::fs::write(&blocked, "").unwrap();
  let under_file = State::new(
    &Env::from_pairs([("XDG_STATE_HOME", blocked.to_str().unwrap())]),
    "vm",
  );
  assert_eq!(under_file.create().unwrap_err().exit, Exit::Failure);
  assert_eq!(under_file.lock().unwrap_err().exit, Exit::Failure);
  let err = write(&under_file.yaml(), "x").unwrap_err();
  assert!(err.message.starts_with("cannot write "), "{}", err.message);
}
