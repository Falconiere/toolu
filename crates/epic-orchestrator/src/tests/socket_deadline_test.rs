use std::time::{Duration, Instant};

use crate::model::World;
use crate::watch::{Kind, Watch};

use super::{github_ready, prompt_ready, stall_ready, wake_after};

#[test]
fn github_retry_after_hold_does_not_spin_the_socket() {
  let mut world = World::new(1_000);
  world.watches.insert(
    "pr:o/r#1".into(),
    Watch::new(
      Kind::Pr {
        key: "one".into(),
        repo: "o/r".into(),
        number: 1,
      },
      1_000,
    ),
  );
  world.github_hold_until_ms = 61_000;
  assert!(!github_ready(&world));
  let wake = wake_after(&world, &[], Instant::now() + Duration::from_secs(90));
  assert!(wake >= Duration::from_secs(59));
  world.now_ms = 61_000;
  assert!(github_ready(&world));
}

#[test]
fn a_queued_prompt_wakes_the_socket_before_the_tick() {
  let mut world = World::new(1_000);
  crate::schedule::enqueue_prompt(&mut world, "a", "hi", "relay", None);
  assert!(!prompt_ready(&world));
  let wake = wake_after(&world, &[], Instant::now() + crate::TICK);
  assert!(wake <= Duration::from_millis(5_000), "{wake:?}");
  world.now_ms = 6_000;
  assert!(prompt_ready(&world));
  // A due prompt is delivered on its own, without a full engine tick.
  assert!(!stall_ready(&world));
}

#[test]
fn an_idle_world_sleeps_until_the_thirty_second_tick() {
  assert_eq!(crate::TICK, Duration::from_secs(30));
  let world = World::new(1_000);
  let wake = wake_after(&world, &[], Instant::now() + crate::TICK);
  assert!(wake > Duration::from_secs(29), "{wake:?}");
  assert!(wake <= crate::TICK, "{wake:?}");
}
