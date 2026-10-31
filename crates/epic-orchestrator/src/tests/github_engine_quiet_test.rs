//! #761 measurement: what quiet scheduled slots of one babysit PR cost in
//! GraphQL requests, all requests and `gh auth token` runs.

use std::time::Instant;

use toolu_github::Config;
use toolu_http_test_support::{Fixture, Reply};
use toolu_pr_babysit::GitHubTick;
use toolu_runtime::env::Env;

use super::babysit_tests::SLOT_MS;
use super::fixture::engine;
use super::fresh_tests::{pending, rest};

/// Quiet scheduled slots after the first, immediate check.
const SLOTS: u64 = 10;

struct Counts {
  graphql: usize,
  requests: usize,
  gh: usize,
  elapsed_ms: u128,
}

/// A `gh` in `dir` that logs each run to `dir/runs` and prints a token.
fn logging_gh(dir: &std::path::Path) {
  use std::os::unix::fs::PermissionsExt as _;
  let gh = dir.join("gh");
  let log = dir.join("runs").display().to_string();
  std::fs::write(&gh, format!("#!/bin/sh\necho . >> '{log}'\necho t-$$\n")).expect("gh");
  std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn measure() -> Counts {
  let dir = tempfile::tempdir().expect("temp");
  logging_gh(dir.path());
  let fixture = Fixture::start().expect("fixture");
  let (body, comments) = pending();
  rest(&fixture, &comments, true);
  fixture
    .route("/graphql", Reply::new(200, body))
    .expect("GraphQL route");
  let (_temp, mut engine, _) = engine(&fixture);
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  let path = dir.path().display().to_string();
  let env = Env::from_pairs([
    ("PATH", path.as_str()),
    ("HTTPS_PROXY", &fixture.proxy_url()),
  ]);
  let tick = GitHubTick::new(config, env);
  let start = Instant::now();
  engine.github_tick(&tick);
  for slot in 1..=SLOTS {
    rest(&fixture, &comments, false);
    engine.world.now_ms = slot * SLOT_MS;
    engine.github_tick(&tick);
  }
  let elapsed_ms = start.elapsed().as_millis();
  let requests = fixture.requests().expect("requests");
  let runs = std::fs::read_to_string(dir.path().join("runs")).unwrap_or_default();
  Counts {
    graphql: requests.iter().filter(|r| r.path == "/graphql").count(),
    requests: requests.len(),
    gh: runs.lines().count(),
    elapsed_ms,
  }
}

#[test]
fn quiet_slots_collect_three_times_in_eleven_checks_with_one_gh_run() {
  let counts = measure();
  eprintln!(
    "quiet_slots: checks={} graphql={} requests={} gh={} elapsed_ms={}",
    SLOTS + 1,
    counts.graphql,
    counts.requests,
    counts.gh,
    counts.elapsed_ms
  );
  assert_eq!(counts.gh, 1, "one token resolution");
  assert_eq!(
    counts.graphql,
    3 * 2,
    "slots 0, 5 and 10 collect, two queries each"
  );
}
