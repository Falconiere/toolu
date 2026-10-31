//! #761 measurement: what quiet scheduled slots of one babysit PR cost in
//! GraphQL and REST requests. The `gh auth token` count lives in pr-babysit's
//! own `engine_tick` test binary, so this one spawns no process.

use std::time::Instant;

use toolu_http_test_support::{Fixture, Reply};

use super::babysit_tests::{SLOT_MS, babysit};
use super::fixture::engine;
use super::fresh_tests::{pending, rest};

/// Quiet scheduled slots after the first, immediate check.
const SLOTS: u64 = 10;

#[test]
fn quiet_slots_collect_three_times_in_eleven_checks() {
  let fixture = Fixture::start().expect("fixture");
  let (body, comments) = pending();
  rest(&fixture, &comments, true);
  fixture
    .route("/graphql", Reply::new(200, body))
    .expect("GraphQL route");
  let (_temp, mut engine, _) = engine(&fixture);
  let tick = babysit(&fixture);
  let start = Instant::now();
  engine.github_tick(&tick);
  for slot in 1..=SLOTS {
    rest(&fixture, &comments, false);
    engine.world.now_ms = slot * SLOT_MS;
    engine.github_tick(&tick);
  }
  let elapsed_ms = start.elapsed().as_millis();
  let requests = fixture.requests().expect("requests");
  let graphql = requests.iter().filter(|r| r.path == "/graphql").count();
  eprintln!(
    "quiet_slots: checks={} graphql={graphql} requests={} elapsed_ms={elapsed_ms}",
    SLOTS + 1,
    requests.len()
  );
  assert_eq!(
    graphql,
    3 * 2,
    "slots 0, 5 and 10 collect, two queries each"
  );
}
