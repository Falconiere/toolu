//! A finished job reaches the engine through its control socket event.

use serde_json::json;
use toolu_engine::resources::job_results;

use super::tests::{open, round};

#[test]
fn job_done_event_journals_one_prompt_and_a_repeat_journals_none() {
  let tmp = tempfile::tempdir().expect("temp");
  let tree = tmp.path().join("work");
  let mut engine = open(tmp.path());
  let issue = crate::logic::ensure_issue(&mut engine.world, "a", "one", "/epic");
  issue.stage = "running".into();
  issue.worktree = Some(tree.display().to_string());
  issue.set_agent_status("idle", 1);
  let mut result = job_results::create(tmp.path(), "a", &tree, "gate").expect("create");
  result.state = "done".into();
  result.exit_code = Some(0);
  job_results::update(tmp.path(), &result).expect("finish");
  let request = json!({"op": "event", "key": "a", "event": "job-done"});
  assert_eq!(round(&mut engine, &request).1["ok"], true);
  assert_eq!(round(&mut engine, &request).1["ok"], true);
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  let prompts: Vec<_> = rows
    .iter()
    .filter(|row| row.kind == "prompt" && row.name == "job")
    .collect();
  assert_eq!(prompts.len(), 1);
  assert!(prompts[0].note.contains(&result.id));
  assert_eq!(
    job_results::pending(tmp.path(), &tree)
      .expect("pending")
      .len(),
    0
  );
}

#[test]
fn job_done_while_working_leaves_the_result_for_an_idle_update() {
  let tmp = tempfile::tempdir().expect("temp");
  let tree = tmp.path().join("work");
  let mut engine = open(tmp.path());
  let issue = crate::logic::ensure_issue(&mut engine.world, "a", "one", "/epic");
  issue.stage = "running".into();
  issue.worktree = Some(tree.display().to_string());
  issue.set_agent_status("working", 1);
  let mut result = job_results::create(tmp.path(), "a", &tree, "gate").expect("create");
  result.state = "done".into();
  result.exit_code = Some(0);
  job_results::update(tmp.path(), &result).expect("finish");
  let request = json!({"op": "event", "key": "a", "event": "job-done"});
  assert_eq!(round(&mut engine, &request).1["ok"], true);
  assert_eq!(
    job_results::pending(tmp.path(), &tree)
      .expect("pending")
      .len(),
    1
  );
  let rows = crate::journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert!(
    !rows
      .iter()
      .any(|row| row.kind == "prompt" && row.name == "job")
  );
  engine
    .world
    .issues
    .get_mut("a")
    .expect("issue")
    .set_agent_status("idle", 2);
  crate::schedule::prompt_jobs(&mut engine, "a").expect("idle prompt");
  assert_eq!(
    job_results::pending(tmp.path(), &tree)
      .expect("pending")
      .len(),
    0
  );
  assert_eq!(
    engine
      .world
      .outbox
      .iter()
      .filter(|step| matches!(step, crate::model::Step::Prompt { .. }))
      .count(),
    1
  );
}
