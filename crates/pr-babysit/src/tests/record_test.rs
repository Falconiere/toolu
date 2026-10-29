use std::path::PathBuf;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_runtime::env::Env;

use super::{Entry, record};
use crate::fixture;
use crate::writes::Slot;

const NOW: &str = "2026-10-11T12:00:00Z";

fn state(dir: &TempDir, edit: impl FnOnce(&mut Value)) -> PathBuf {
  let path = dir.path().join("state.json");
  let mut value = fixture::json("states/toolu-165-initial.json");
  edit(&mut value);
  std::fs::write(&path, value.to_string()).unwrap();
  path
}

fn read(path: &PathBuf) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn a_round_rotates_the_keys_records_the_rejection_and_caps_fix_attempts() {
  let dir = TempDir::new().unwrap();
  let keys = json!(["plugins/toolu/hooks/session-start.sh:17:real-key"]);
  let path = state(&dir, |value| {
    value["pr"]["botFindingKeys"] = keys.clone();
    value["pr"]["fixAttempts"] = json!(4);
    value["fixer"] = json!({"status": "done"});
  });
  let env = Env::default();
  let slot = Slot {
    path: &path,
    env: &env,
    now: NOW,
  };
  let round = Entry::Round {
    had_rejection: true,
    fix_pushed: true,
  };
  let first = record(&slot, &round).unwrap();
  assert_eq!(first["lastRoundFindingKeys"], keys);
  assert_eq!(first["fixAttempts"], 5);
  let after = read(&path);
  assert_eq!(after["lastRound"]["hadRejection"], true);
  assert_eq!(after["lastRound"]["fixPushed"], true);
  assert_eq!(after["lastRound"]["headSha"], after["pr"]["headSha"]);
  assert_eq!(after["fixer"], Value::Null, "a settled fixer is cleared");
  let second = Entry::Round {
    had_rejection: false,
    fix_pushed: true,
  };
  record(&slot, &second).unwrap();
  let after = read(&path);
  assert_eq!(after["pr"]["fixAttempts"], 5);
  assert_eq!(after["pr"]["lastRoundHadRejection"], false);
}

fn record_empty_round(path: &PathBuf) {
  let env = Env::default();
  let slot = Slot {
    path,
    env: &env,
    now: NOW,
  };
  record(
    &slot,
    &Entry::Round {
      had_rejection: false,
      fix_pushed: false,
    },
  )
  .unwrap();
}

#[test]
fn a_round_keeps_a_running_fixer() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir, |value| value["fixer"] = json!({"status": "running"}));
  record_empty_round(&path);
  assert_eq!(read(&path)["fixer"]["status"], "running");
}

#[test]
fn flag_injection_and_terminal_status_persist_under_the_slot_lock() {
  let dir = TempDir::new().unwrap();
  let path = state(&dir, |_| {});
  let env = Env::default();
  let slot = Slot {
    path: &path,
    env: &env,
    now: NOW,
  };
  record(&slot, &Entry::FlagInjection("PRRT_example")).unwrap();
  assert_eq!(
    read(&path)["actions"]["flagged"]["PRRT_example"]["reason"],
    "injection"
  );
  let done = record(&slot, &Entry::Status("complete")).unwrap();
  assert_eq!(done["status"], "complete");
  let after = read(&path);
  assert_eq!(
    (after["status"].as_str(), after["statusChangedAt"].as_str()),
    (Some("complete"), Some(NOW))
  );
  assert!(!dir.path().join("state.json.lock").exists());
}

#[test]
fn a_missing_state_fails_without_creating_a_file() {
  let dir = TempDir::new().unwrap();
  let path = dir.path().join("state.json");
  let env = Env::default();
  let slot = Slot {
    path: &path,
    env: &env,
    now: NOW,
  };
  let failure = record(&slot, &Entry::Status("complete")).err().unwrap();
  assert_eq!(failure.code, "state_malformed");
  assert!(failure.message.contains("run toolu babysit tick first"));
  assert!(!path.exists());
}

#[test]
fn a_recorded_round_preserves_remote_fixer_identity_for_the_next_attempt() {
  let dir = TempDir::new().unwrap();
  let fixer = json!({"status":"failed", "groups":[{"status":"failed", "hcName":"old"}]});
  let path = state(&dir, |value| {
    value["fixer"] = fixer.clone();
    value["herdrWorktree"] = json!({"remote":{"run":"old"}});
  });
  record_empty_round(&path);
  let after = read(&path);
  assert_eq!(after["fixer"], Value::Null);
  assert_eq!(after["herdrWorktree"]["remote"]["lastFixer"], fixer);
}
