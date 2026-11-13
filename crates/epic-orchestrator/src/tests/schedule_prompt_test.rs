//! Queued worker prompts: bursts collapse to one debounced prompt that keeps
//! every distinct text, and entries for dead agents are dropped on flush.

use super::{enqueue_prompt, flush_prompts, next_prompt_ms};
use crate::logic::ensure_issue;
use crate::model::{Step, World};
use crate::server::{Engine, Fault};

fn idle_job_engine() -> (tempfile::TempDir, Engine) {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine =
    Engine::open(crate::paths::Paths::at(tmp.path()), None, Fault::None).expect("open");
  let issue = ensure_issue(&mut engine.world, "a", "one", "/epic");
  issue.stage = "running".into();
  issue.worktree = Some(tmp.path().join("work").display().to_string());
  issue.set_agent_status("idle", 1);
  (tmp, engine)
}

#[test]
fn job_done_claims_each_finished_result_for_an_idle_worker_once() {
  use toolu_engine::resources::job_results;

  let (tmp, mut engine) = idle_job_engine();
  let tree = tmp.path().join("work");
  for (label, code, output) in [("one", 0, ""), ("two", 3, "failed output")] {
    let mut record = job_results::create(tmp.path(), "a", &tree, label).expect("create");
    record.state = "done".into();
    record.exit_code = Some(code);
    record.output = output.into();
    job_results::update(tmp.path(), &record).expect("finish");
  }
  super::prompt_jobs(&mut engine, "a").expect("prompt");
  super::prompt_jobs(&mut engine, "a").expect("dedupe");
  let prompts: Vec<&str> = engine
    .world
    .outbox
    .iter()
    .filter_map(|step| match step {
      Step::Prompt { text, .. } => Some(text.as_str()),
      Step::Journal(_)
      | Step::Call { .. }
      | Step::Status { .. }
      | Step::Issue { .. }
      | Step::Notify { .. } => None,
    })
    .collect();
  assert_eq!(prompts.len(), 2);
  assert!(prompts[0].contains("job "));
  assert!(prompts.iter().any(|text| text.contains("failed output")));
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
      .filter(
        |step| matches!(step, Step::Journal(row) if row.kind == "prompt" && row.name == "job")
      )
      .count(),
    2
  );
}

#[test]
fn job_done_leaves_a_bad_result_unclaimed_without_a_prompt() {
  let (tmp, mut engine) = idle_job_engine();
  std::fs::create_dir_all(tmp.path().join("jobs/results")).expect("results");
  std::fs::write(tmp.path().join("jobs/results/bad.json"), "broken").expect("bad");
  assert!(super::prompt_jobs(&mut engine, "a").is_err());
  assert!(engine.world.outbox.is_empty());
}

#[test]
fn job_done_does_not_claim_a_result_after_the_worker_is_gone() {
  use toolu_engine::resources::job_results;

  let (tmp, mut engine) = idle_job_engine();
  let tree = tmp.path().join("work");
  engine.world.issues.get_mut("a").expect("issue").stage = "merged".into();
  let mut record = job_results::create(tmp.path(), "a", &tree, "late gate").expect("create");
  record.state = "done".into();
  record.exit_code = Some(0);
  job_results::update(tmp.path(), &record).expect("finish");
  super::prompt_jobs(&mut engine, "a").expect("late event");
  assert_eq!(
    job_results::pending(tmp.path(), &tree)
      .expect("pending")
      .len(),
    1
  );
  assert!(engine.world.outbox.is_empty());
}

#[test]
fn a_burst_of_queued_prompts_flushes_once_with_every_distinct_text() {
  let mut world = World::new(0);
  let issue = ensure_issue(&mut world, "a", "one", "/epic");
  issue.stage = "running".to_owned();
  for n in 0..10 {
    enqueue_prompt(&mut world, "a", &format!("n{n}"), "relay", None);
  }
  // A repeat of a queued text adds nothing.
  enqueue_prompt(&mut world, "a", "n3", "relay", None);
  assert_eq!(next_prompt_ms(&world), Some(5_000));
  world.now_ms = 4_999;
  flush_prompts(&mut world);
  assert!(world.outbox.is_empty());
  world.now_ms = 5_000;
  flush_prompts(&mut world);
  let prompts: Vec<(String, String)> = world
    .outbox
    .iter()
    .filter_map(|step| match step {
      Step::Prompt { key, text, .. } => Some((key.clone(), text.clone())),
      Step::Journal(_)
      | Step::Call { .. }
      | Step::Status { .. }
      | Step::Issue { .. }
      | Step::Notify { .. } => None,
    })
    .collect();
  assert_eq!(
    prompts,
    [("a".to_owned(), "n0 n1 n2 n3 n4 n5 n6 n7 n8 n9".to_owned())]
  );
  assert!(world.prompt_queue.is_empty());
  let journals: Vec<_> = world
    .outbox
    .iter()
    .filter_map(|step| match step {
      Step::Journal(row) if row.kind == "prompt" => Some(row.name.clone()),
      Step::Journal(_)
      | Step::Call { .. }
      | Step::Status { .. }
      | Step::Issue { .. }
      | Step::Prompt { .. }
      | Step::Notify { .. } => None,
    })
    .collect();
  assert_eq!(journals, ["relay"]);
}

#[test]
fn a_flushed_prompt_for_a_dead_agent_is_dropped() {
  let mut world = World::new(0);
  let issue = ensure_issue(&mut world, "a", "one", "/epic");
  issue.stage = "merged".to_owned();
  enqueue_prompt(&mut world, "a", "hi", "relay", None);
  world.now_ms = 5_000;
  flush_prompts(&mut world);
  assert!(world.outbox.is_empty());
  assert!(world.prompt_queue.is_empty());
}
