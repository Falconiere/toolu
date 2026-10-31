//! The engine's babysit tick keeps one GitHub client (#761): `gh auth token`
//! runs once, again only after a `401`, and a failed build is not kept. These
//! tests spawn a real `gh` script, so they run in their own test binary, apart
//! from the crate's many concurrent unit tests and the processes they start.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickRequest};
use toolu_github::Config;
use toolu_http_test_support::{Fixture, Reply};
use toolu_pr_babysit::GitHubTick;
use toolu_runtime::env::Env;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const HEAD_PATH: &str = "/repos/o/r/pulls/1";

/// Serve an open PR `o/r#1` with a running check and no threads or comments.
fn serve(fixture: &Fixture) -> Result<()> {
  let check = json!({"__typename": "CheckRun", "name": "rust", "status": "IN_PROGRESS",
    "conclusion": null, "detailsUrl": "https://ci.example/1", "startedAt": "2026-10-11T12:00:00Z",
    "completedAt": null, "checkSuite": {"workflowRun": {"workflow": {"name": "tests"}}}});
  let pr = json!({"number": 1, "title": "feat: one", "url": "https://github.com/o/r/pull/1",
    "author": {"login": "worker"}, "state": "OPEN", "baseRefName": "main",
    "headRefName": "feat/one", "headRefOid": SHA, "mergeable": "MERGEABLE", "reviewDecision": null,
    "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [check]}}}}]},
    "reviewThreads": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": []}});
  let page = json!({"data": {"rateLimit": {"cost": 1}, "repository": {"pullRequest": pr}}});
  fixture.route("/graphql", Reply::new(200, page.to_string()))?;
  fixture.route(HEAD_PATH, Reply::new(200, head()))?;
  for path in [
    "/repos/o/r/issues/1/comments?per_page=100",
    "/repos/o/r/pulls/1/reviews?per_page=100",
  ] {
    fixture.route(path, Reply::new(200, "[]"))?;
  }
  Ok(())
}

fn head() -> String {
  json!({"head": {"sha": SHA}}).to_string()
}

/// A `gh` in `dir` that prints a new token per run and logs each run.
fn logging_gh(dir: &Path) -> Result<PathBuf> {
  let log = dir.join("runs");
  let gh = dir.join("gh");
  let script = format!(
    "#!/bin/sh\necho run >> '{}'\necho \"tok-$$\"\n",
    log.display()
  );
  std::fs::write(&gh, script)?;
  std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755))?;
  Ok(log)
}

fn runs(log: &Path) -> usize {
  std::fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

/// An engine tick of `o/r#1` whose environment is `PATH=dir` with no `GH_TOKEN`.
fn engine_tick(dir: &Path, fixture: &Fixture) -> (GitHubTick, TickRequest) {
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  let path = dir.display().to_string();
  let env = Env::from_pairs([
    ("PATH", path.as_str()),
    ("HTTPS_PROXY", &fixture.proxy_url()),
  ]);
  let request = TickRequest {
    repo: "o/r".into(),
    number: 1,
    state_file: dir.join("o__r-1.json"),
    now: Some("2026-10-11T12:00:00Z".into()),
  };
  (GitHubTick::new(config, env), request)
}

#[test]
fn an_engine_tick_resolves_the_token_once_and_again_only_after_a_401() {
  let dir = TempDir::new().expect("temp dir");
  let log = logging_gh(dir.path()).expect("gh script");
  let fixture = Fixture::start().expect("fixture");
  serve(&fixture).expect("routes");
  let (tick, request) = engine_tick(dir.path(), &fixture);
  for _ in 0..3 {
    tick.tick(&request).expect("tick");
  }
  assert_eq!(runs(&log), 1, "one token resolution for three ticks");
  let unauthorized = vec![Reply::new(401, "{}"), Reply::new(200, head())];
  fixture
    .sequence(HEAD_PATH, unauthorized)
    .expect("401 route");
  tick.tick(&request).expect("tick after 401");
  assert_eq!(runs(&log), 2, "a 401 re-reads the token once");
}

#[test]
fn a_failed_client_build_is_not_kept_and_the_next_tick_retries() {
  let dir = TempDir::new().expect("temp dir");
  let fixture = Fixture::start().expect("fixture");
  serve(&fixture).expect("routes");
  let (tick, request) = engine_tick(dir.path(), &fixture);
  let failed = tick.tick(&request);
  assert!(
    matches!(&failed, Err(LinkError::Failed(message)) if message.starts_with("gh_unavailable")),
    "{failed:?}"
  );
  let log = logging_gh(dir.path()).expect("gh script");
  tick.tick(&request).expect("tick with gh");
  assert_eq!(runs(&log), 1);
}

#[test]
fn a_failed_collection_drops_the_kept_client_so_the_token_is_read_again() {
  let dir = TempDir::new().expect("temp dir");
  let log = logging_gh(dir.path()).expect("gh script");
  let fixture = Fixture::start().expect("fixture");
  serve(&fixture).expect("routes");
  let (tick, request) = engine_tick(dir.path(), &fixture);
  tick.tick(&request).expect("first tick");
  assert_eq!(runs(&log), 1);
  let forbidden = vec![
    Reply::new(403, r#"{"message":"Resource not accessible"}"#),
    Reply::new(200, head()),
  ];
  fixture.sequence(HEAD_PATH, forbidden).expect("403 route");
  assert!(tick.tick(&request).is_err(), "a rescoped token answers 403");
  tick.tick(&request).expect("tick after 403");
  assert_eq!(
    runs(&log),
    2,
    "the failure dropped the client, so gh ran again"
  );
}
