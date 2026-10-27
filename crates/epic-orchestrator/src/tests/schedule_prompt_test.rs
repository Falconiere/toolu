//! Queued worker prompts: bursts collapse to one debounced prompt that keeps
//! every distinct text, and entries for dead agents are dropped on flush.

use super::{enqueue_prompt, flush_prompts, next_prompt_ms};
use crate::logic::ensure_issue;
use crate::model::{Step, World};

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
