use std::path::PathBuf;

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
  let state = State::new(&env, "vm");
  let held = state.lock().unwrap();
  let busy = state.lock().unwrap_err();
  assert_eq!(busy.exit, Exit::TempFail);
  assert!(
    busy
      .message
      .starts_with("another toolu sandbox command is changing vm")
  );
  drop(held);
  assert!(state.lock().is_ok());
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
