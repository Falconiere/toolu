//! The queue half of the scripted-epic budgets (#451): real worktrees, real
//! engine rebases, a conflict handed to its worker, merges and an orchestrator
//! `wait`, measured from the engine's journal. The GitHub half (keep-going
//! checks, the green verdict) is `github_engine`'s `metrics_tests`.

use std::sync::mpsc;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_state::time::epoch_millis;

use crate::journal::{self, Record};
use crate::metrics::{Query, budget_rows, document};
use crate::queue_rig::{Checks, QueueRig};
use crate::server::Engine;

fn statuses(engine: &Engine) -> Vec<(String, String)> {
  let query = Query {
    epic: None,
    since_ms: None,
    now_ms: epoch_millis(SystemTime::now()),
  };
  let doc = document(&engine.paths.journal_dir(), &query).expect("metrics");
  budget_rows(&doc)
    .iter()
    .map(|row| {
      let text = |field: &str| row[field].as_str().unwrap_or("").to_owned();
      (text("id"), text("status"))
    })
    .collect()
}

fn status(engine: &Engine, id: &str) -> String {
  statuses(engine)
    .into_iter()
    .find(|(row, _)| row == id)
    .map(|(_, status)| status)
    .unwrap_or_default()
}

/// Three PRs merge in order; B and C are rebased by the engine.
fn clean_queue() -> QueueRig {
  let mut rig = QueueRig::new(&[2, 3, 4]);
  for number in [2, 3, 4] {
    rig.work(
      number,
      &format!("f{number}.txt"),
      &format!("child {number}\n"),
    );
  }
  rig.serve(2, Checks::Green, false);
  rig.land(2);
  for number in [3, 4] {
    rig.serve(number, Checks::Green, false);
  }
  rig.ready(&[2, 3, 4]);
  rig.serve(3, Checks::Running, false);
  rig.checked(3);
  rig.serve(3, Checks::Green, true);
  rig.land(3);
  rig.serve(4, Checks::Green, false);
  rig.checked(3);
  rig.serve(4, Checks::Green, false);
  rig.checked(4);
  rig
}

/// The orchestrator waits and is woken with a judgment.
fn wait_for_judgment(engine: &mut Engine) -> Value {
  crate::schedule::note_event(&mut engine.world, "o-r-2", "blocked");
  let (tx, rx) = mpsc::channel();
  let mut waiters = Vec::new();
  let request = json!({"op": "wait", "max_seconds": 30});
  crate::dispatch::dispatch(engine, &request, &tx, &mut waiters, crate::PROTOCOL);
  rx.recv().expect("wait reply")
}

#[test]
fn the_scripted_queue_passes_its_budgets() {
  let mut rig = clean_queue();
  let reply = wait_for_judgment(&mut rig.engine);
  assert!(reply.get("kind").is_some(), "{reply}");
  let all = statuses(&rig.engine);
  for id in [
    "judgment-free-wakeups",
    "rebase-without-conflict",
    "ready-to-merged",
  ] {
    assert!(
      all.contains(&(id.to_owned(), "pass".to_owned())),
      "{id}: {all:?}"
    );
  }
  assert!(all.iter().all(|(_, status)| status != "fail"), "{all:?}");
}

#[test]
fn a_conflict_hand_off_licenses_its_rebase_prompt_and_an_unlicensed_one_fails() {
  let mut rig = QueueRig::new(&[2, 3]);
  rig.work(2, "a.txt", "from two\n");
  rig.work(3, "a.txt", "from three\n");
  rig.serve(2, Checks::Green, false);
  rig.land(2);
  rig.serve(3, Checks::Green, false);
  rig.ready(&[2, 3]);
  assert_eq!(status(&rig.engine, "rebase-without-conflict"), "pass");
  let dir = rig.engine.paths.journal_dir();
  let lock = rig.engine.paths.journal_lock();
  let planted = Record::new("prompt", "rebase", "o-r-3", "", "REBASE");
  journal::append(&dir, &lock, &planted, SystemTime::now()).expect("plant");
  assert_eq!(status(&rig.engine, "rebase-without-conflict"), "fail");
  let timeout = Record::new("action", "wait-reply", "", "", "timeout");
  journal::append(&dir, &lock, &timeout, SystemTime::now()).expect("plant");
  assert_eq!(status(&rig.engine, "judgment-free-wakeups"), "fail");
}
