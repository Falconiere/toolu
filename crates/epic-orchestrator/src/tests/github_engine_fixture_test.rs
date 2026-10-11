use serde_json::{Value, json};
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, GraphQlUsage, TickDecision, TickReport, TickRequest};
use toolu_github::{Client, Config, Error};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use crate::model::Issue;
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::watch;

const QUERY: &str = "query($owner: String!, $repo: String!, $number: Int!) { \
  rateLimit { cost } repository(owner: $owner, name: $repo) { \
  pullRequest(number: $number) { reviewThreads(first: 100) { nodes { isResolved } } \
  commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } \
  comments(last: 20) { nodes { body } } } } }";

pub(super) struct FullTick(Client);

impl BabysitTick for FullTick {
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError> {
    let (owner, repo) = request
      .repo
      .split_once('/')
      .ok_or_else(|| LinkError::Failed("repo".into()))?;
    let reply = self
      .0
      .graphql(
        QUERY,
        &json!({"owner":owner,"repo":repo,"number":request.number}),
      )
      .map_err(link_error)?;
    let pr = reply
      .data
      .pointer("/repository/pullRequest")
      .ok_or_else(|| LinkError::Failed("PR".into()))?;
    let green = pr
      .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
      .and_then(Value::as_str)
      == Some("SUCCESS");
    let approved = pr
      .pointer("/comments/nodes")
      .and_then(Value::as_array)
      .is_some_and(|rows| {
        rows.iter().any(|row| {
          row["body"]
            .as_str()
            .is_some_and(|body| body.contains("approved, 0 findings"))
        })
      });
    let resolved = pr
      .pointer("/reviewThreads/nodes")
      .and_then(Value::as_array)
      .is_some_and(|rows| rows.iter().all(|row| row["isResolved"] == true));
    Ok(TickReport {
      decision: if green && approved && resolved {
        TickDecision::Success
      } else {
        TickDecision::KeepGoing
      },
      result: reply.data,
      graphql: reply.cost.points.map(|points| GraphQlUsage {
        points,
        remaining: reply.cost.rate.remaining,
        reset_at: reply.cost.rate.reset,
      }),
    })
  }
}

fn link_error(err: Error) -> LinkError {
  match err {
    Error::RateLimited {
      retry_after, rate, ..
    } => LinkError::RateLimited {
      retry_after,
      remaining: rate.remaining,
      reset_at: rate.reset,
    },
    err @ (Error::Token(_)
    | Error::Config(_)
    | Error::Unauthorized
    | Error::Status { .. }
    | Error::GraphQl(_)
    | Error::Transport(_)
    | Error::Decode(_)) => LinkError::Failed(err.to_string()),
  }
}

fn client(fixture: &Fixture) -> Client {
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  Client::new(
    config,
    &Env::from_pairs([
      ("GH_TOKEN", "engine-token"),
      ("HTTPS_PROXY", &fixture.proxy_url()),
    ]),
  )
  .expect("client")
}

pub(super) fn pr_reply() -> String {
  json!({"state":"open","merged":false,"head":{"sha":"abc"},"base":{"ref":"main"}}).to_string()
}

pub(super) fn rest_routes(fixture: &Fixture) {
  let root = "/repos/o/r";
  for (path, body) in [
    (format!("{root}/pulls/1"), pr_reply()),
    (
      format!("{root}/commits/abc/check-runs?per_page=100"),
      r#"{"check_runs":[]}"#.into(),
    ),
    (format!("{root}/commits/abc/status"), "{}".into()),
    (
      format!("{root}/issues/1/comments?per_page=100"),
      "[]".into(),
    ),
    (format!("{root}/pulls/1/comments?per_page=100"), "[]".into()),
    (format!("{root}/pulls/1/reviews?per_page=100"), "[]".into()),
  ] {
    fixture
      .sequence(
        &path,
        vec![
          Reply::new(200, body)
            .header("ETag", &format!("\"{}\"", path.len()))
            .header("X-RateLimit-Used", "6"),
          Reply::new(304, "").header("X-RateLimit-Used", "6"),
        ],
      )
      .expect("REST route");
  }
}

pub(super) fn graphql_body(green: bool) -> String {
  json!({"data": {"rateLimit":{"cost":1}, "repository":{"pullRequest":{
    "reviewThreads":{"nodes":[{"isResolved":green}]},
    "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":if green {"SUCCESS"} else {"PENDING"}}}}]},
    "comments":{"nodes":[{"body":if green {"approved, 0 findings"} else {"review running"}}]}
  }}}}).to_string()
}

pub(super) fn engine(fixture: &Fixture) -> (tempfile::TempDir, Engine, FullTick) {
  let temp = tempfile::tempdir().expect("temp");
  std::fs::create_dir_all(temp.path().join("status")).expect("status dir");
  let mut engine = Engine::open(Paths::at(temp.path()), None, Fault::None).expect("open");
  engine.world.now_ms = 0;
  let mut issue = Issue::blank("one", "epic", &temp.path().display().to_string(), 0);
  issue.repo = "o/r".into();
  issue.phase = "babysit".into();
  issue.stage = "running".into();
  issue.pr = Some(1);
  engine.world.issues.insert("one".into(), issue);
  watch::sync(&mut engine.world);
  engine.github = Some(client(fixture));
  (temp, engine, FullTick(client(fixture)))
}

#[test]
fn a_registered_pr_fixture_emits_the_six_rest_inputs_and_one_graphql_tick() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route("/graphql", Reply::new(200, graphql_body(false)))
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 7);
  assert_eq!(requests[6].path, "/graphql");
}
