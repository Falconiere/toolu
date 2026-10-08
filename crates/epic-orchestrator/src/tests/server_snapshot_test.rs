use toolu_github::RateLimit;

use crate::paths::Paths;
use crate::server::{Engine, Fault};

#[test]
fn github_budget_and_hold_survive_a_watch_snapshot() {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  let mut engine = Engine::open(paths.clone(), None, Fault::None).expect("open");
  engine.world.github_hold_until_ms = 60_000;
  engine.world.rest_rate = Some(RateLimit {
    remaining: Some(91),
    ..RateLimit::default()
  });
  engine.world.graphql_remaining = Some(18);
  engine.world.graphql_points = 4;
  engine.persist_watch().expect("save");
  let recovered = Engine::open(paths, None, Fault::None).expect("restart");
  assert_eq!(recovered.world.github_hold_until_ms, 60_000);
  assert_eq!(
    recovered.world.rest_rate.and_then(|rate| rate.remaining),
    Some(91)
  );
  assert_eq!(recovered.world.graphql_remaining, Some(18));
  assert_eq!(recovered.world.graphql_points, 4);
}
