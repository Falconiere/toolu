use std::time::{Duration, Instant};

use crate::model::World;
use crate::watch::{Kind, Watch};

use super::{github_ready, wake_after};

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
