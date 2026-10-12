use crate::queue_rig::{Checks, QueueRig};
use crate::scenario_rig::key;

fn stage(rig: &QueueRig, number: u64) -> String {
  rig.engine.world.issues[&key(number)].stage.clone()
}

fn engine_rebases(rig: &QueueRig) -> Vec<(String, String)> {
  rig
    .queue_journal()
    .into_iter()
    .filter(|(name, _, _)| name == "rebase")
    .map(|(_, key, note)| (key, note))
    .collect()
}

#[test]
fn queue_scenario_three_ready_prs_merge_in_order_with_two_engine_rebases() {
  let mut rig = QueueRig::new(&[2, 3, 4]);
  for number in [2, 3, 4] {
    rig.work(number, &format!("f{number}.txt"), &format!("child {number}\n"));
  }
  // A is up to date and green: GitHub merges it at the engine's call.
  rig.serve(2, Checks::Green, false);
  rig.land(2);
  for number in [3, 4] {
    rig.serve(number, Checks::Green, false);
  }
  let (b_before, c_before) = (rig.head(3), rig.head(4));
  rig.ready(&[2, 3, 4]);
  assert!(matches!(stage(&rig, 2).as_str(), "merged" | "cleaning"), "{}", stage(&rig, 2));
  assert_ne!(rig.head(3), b_before, "the engine rebased B and pushed it");
  assert!(!rig.gated(4), "C waits without a gate while B is the head");
  assert_eq!(rig.head(4), c_before);
  // B's CI runs on the new head: auto-merge is armed, nothing merges.
  rig.serve(3, Checks::Running, false);
  rig.checked(3);
  assert!(rig.engine.world.issues[&key(3)].auto_armed);
  assert_eq!(rig.merge_calls().len(), 1);
  // B goes green and merges; C is rebased next.
  rig.serve(3, Checks::Green, true);
  rig.land(3);
  rig.serve(4, Checks::Green, false);
  rig.checked(3);
  assert_ne!(rig.head(4), c_before, "the engine rebased C");
  rig.serve(4, Checks::Green, false);
  rig.checked(4);
  assert_eq!(
    rig.merge_calls(),
    ["/repos/o/r/pulls/102/merge", "/repos/o/r/pulls/103/merge", "/repos/o/r/pulls/104/merge"]
  );
  let rebases = engine_rebases(&rig);
  assert_eq!(rebases.len(), 2, "{rebases:?}");
  for ((issue, note), number) in rebases.iter().zip([3, 4]) {
    assert_eq!(issue, &key(number));
    assert!(note.starts_with(r#"{"by":"engine","head":"#), "{note}");
    assert!(note.ends_with(r#""round":1}"#), "{note}");
  }
  assert!(rig.queue_journal().iter().all(|(name, _, _)| name != "rebase-skipped"));
  let prompts = rig.queue_journal().into_iter().filter(|(_, _, note)| note == "REBASE").count();
  assert_eq!(prompts, 0, "no worker was asked to rebase");
  assert!(rig.scenario.journal().iter().all(|row| row.kind != "prompt"));
  for number in [2, 3, 4] {
    assert!(matches!(stage(&rig, number).as_str(), "merged" | "cleaning"), "{number}");
  }
  assert_eq!(rig.record(4)["rebase_rounds"], serde_json::json!({"engine": 1, "worker": 0}));
}

#[test]
fn queue_scenario_a_conflict_is_aborted_and_handed_to_the_worker_once() {
  let mut rig = QueueRig::new(&[2, 3]);
  rig.work(2, "a.txt", "from two\n");
  rig.work(3, "a.txt", "from three\n");
  std::fs::write(rig.worktree(3).join("notes.txt"), "untracked\n").expect("untracked");
  rig.serve(2, Checks::Green, false);
  rig.land(2);
  rig.serve(3, Checks::Green, false);
  let tree = rig.worktree(3);
  let read = |name: &str| std::fs::read(tree.join(name)).expect("read");
  let before = (read("a.txt"), read("notes.txt"), rig.head(3));
  rig.ready(&[2, 3]);
  let after = (read("a.txt"), read("notes.txt"), rig.head(3));
  assert_eq!(after, before, "the worktree is byte-identical and nothing was pushed");
  let snapshot = crate::support::git(&tree, &["show", "refs/epic-wip/o-r-3:notes.txt"]);
  assert_eq!(snapshot, "untracked");
  let issue = &rig.engine.world.issues[&key(3)];
  assert_eq!((issue.phase.as_str(), issue.queued_at_ms), ("rebasing", None));
  let journal = rig.scenario.journal();
  let rows = |kind: &str| -> Vec<String> {
    journal
      .iter()
      .filter(|row| row.kind == kind && row.name == "rebase")
      .map(|row| row.note.clone())
      .collect()
  };
  assert_eq!(rows("action"), [r#"{"by":"worker","round":1}"#]);
  assert_eq!(rows("prompt"), ["REBASE"], "the worker is asked once");
  assert_eq!(rig.record(3)["rebase_rounds"], serde_json::json!({"engine": 0, "worker": 1}));
  assert_eq!(rig.merge_calls().len(), 1);
}

#[test]
fn queue_scenario_a_base_move_disarms_the_waiting_head_and_rebases_it_again() {
  let mut rig = QueueRig::new(&[2]);
  rig.work(2, "f2.txt", "two\n");
  rig.serve(2, Checks::Running, false);
  rig.ready(&[2]);
  assert!(rig.engine.world.issues[&key(2)].auto_armed);
  let armed_head = rig.head(2);
  rig.push_main("human.txt");
  rig.serve(2, Checks::Running, true);
  let change = crate::github_probe::Change::BaseMoved {
    repo: "o/r".into(),
    branch: "main".into(),
  };
  crate::github_engine::note_change(&mut rig.engine.world, change);
  assert_eq!(rig.engine.pump().expect("pump"), crate::server::Stop::Idle);
  let mutations: Vec<String> = rig
    .scenario
    .github
    .requests()
    .expect("requests")
    .iter()
    .map(|request| String::from_utf8_lossy(&request.body).into_owned())
    .filter(|body| body.contains("AutoMerge"))
    .collect();
  assert!(mutations.last().is_some_and(|body| body.contains("disablePullRequestAutoMerge")));
  assert_ne!(rig.head(2), armed_head, "rebased again before any merge");
  assert!(rig.merge_calls().is_empty());
  assert_eq!(engine_rebases(&rig).len(), 1);
}
