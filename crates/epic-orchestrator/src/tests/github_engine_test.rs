use serde_json::{Value, json};
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, GraphQlUsage, TickDecision, TickReport, TickRequest};
use toolu_github::{Client, Config};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{cause_name, rearm};
use crate::disk::read_value;
use crate::journal;
use crate::model::{Issue, Step, World};
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::status;
use crate::watch::{self, Cause, Kind, Watch};

const QUERY: &str = "query($owner: String!, $repo: String!, $number: Int!) { \
  rateLimit { cost } repository(owner: $owner, name: $repo) { \
  pullRequest(number: $number) { reviewThreads(first: 100) { nodes { isResolved } } \
  commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } \
  comments(last: 20) { nodes { body } } } } }";

struct FullTick(Client);

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
      .map_err(|err| LinkError::Failed(err.to_string()))?;
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

fn pr_reply() -> String {
  json!({"state":"open","merged":false,"head":{"sha":"abc"},"base":{"ref":"main"}}).to_string()
}

fn rest_routes(fixture: &Fixture) {
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

fn graphql_body(green: bool) -> String {
  json!({"data": {"rateLimit":{"cost":1}, "repository":{"pullRequest":{
    "reviewThreads":{"nodes":[{"isResolved":green}]},
    "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":if green {"SUCCESS"} else {"PENDING"}}}}]},
    "comments":{"nodes":[{"body":if green {"approved, 0 findings"} else {"review running"}}]}
  }}}}).to_string()
}

fn engine(fixture: &Fixture) -> (tempfile::TempDir, Engine, FullTick) {
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
fn github_watch_full_tick_promotes_green_pr_without_worker_prompt() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .sequence(
      "/graphql",
      vec![
        Reply::new(200, graphql_body(false)).header("X-RateLimit-Remaining", "4999"),
        Reply::new(200, graphql_body(true)).header("X-RateLimit-Remaining", "4998"),
      ],
    )
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].phase, "babysit");
  engine.world.now_ms = 180_000;
  engine.github_tick(&tick);
  assert_eq!(engine.world.issues["one"].phase, "ready");
  assert_eq!(engine.world.graphql_points, 2);
  assert!(
    engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Journal(row) if row.name == "github-babysit-ready"))
  );
  assert!(
    !engine
      .world
      .outbox
      .iter()
      .any(|step| matches!(step, Step::Journal(row) if row.name == "prompt-intent"))
  );
  let view = status::document(&engine.world, true, None);
  assert_eq!(view["github"]["graphql"]["points"], 2);
  assert_eq!(view["issues"][0]["github"]["lastCheckAt"], 180_000);
  assert_eq!(view["issues"][0]["github"]["nextCheckAt"], 360_000);
  engine.flush().expect("flush");
  let rows = journal::tail(&engine.paths.journal_dir(), engine.now).expect("journal");
  assert!(rows.iter().any(|row| row.name == "github-babysit-ready"));
  let saved = read_value(&engine.paths.root.join("status/one.json")).expect("status");
  assert_eq!(saved["phase"], "ready");
}

#[test]
fn github_budget_idle_hour_keeps_rest_primary_flat_and_graphql_separate() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .route(
      "/graphql",
      Reply::new(200, graphql_body(false)).header("X-RateLimit-Remaining", "4900"),
    )
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  for slot in 0..=20 {
    engine.world.now_ms = slot * 180_000;
    engine.github_tick(&tick);
  }
  let requests = fixture.requests().expect("requests");
  let rest: Vec<_> = requests
    .iter()
    .filter(|request| request.path != "/graphql")
    .collect();
  let conditional = rest
    .iter()
    .filter(|request| request.headers.contains_key("if-none-match"))
    .count();
  assert_eq!(rest.len(), 126);
  assert!(conditional * 100 >= 95 * rest.len());
  assert_eq!(engine.world.rest_points, 6);
  assert_eq!(engine.world.graphql_points, 21);
  assert_scheduled_budget(&engine.world);
}

fn assert_scheduled_budget(world: &World) {
  let scheduled: Vec<_> = world
    .outbox
    .iter()
    .filter_map(|step| match step {
      Step::Journal(row) => Some(row),
      Step::Call { .. } | Step::Status { .. } | Step::Issue { .. } => None,
    })
    .filter(|row| row.name == "github-check" && row.note.contains("\"cause\":\"scheduled\""))
    .collect();
  assert_eq!(scheduled.len(), 20);
  let points: u64 = scheduled
    .iter()
    .map(|row| {
      serde_json::from_str::<Value>(&row.note).expect("note")["graphqlPoints"]
        .as_u64()
        .expect("points")
    })
    .sum();
  assert_eq!(points, 20);
  assert!(
    scheduled
      .iter()
      .all(|row| row.note.contains("\"restNotModified\":6"))
  );
}

#[test]
fn github_retry_after_suppresses_requests_until_the_next_fixed_slot() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  fixture
    .sequence(
      "/repos/o/r/pulls/1",
      vec![
        Reply::new(429, r#"{"message":"secondary rate limit"}"#)
          .header("Retry-After", "60")
          .header("X-RateLimit-Remaining", "40"),
        Reply::new(200, pr_reply()).header("ETag", "\"pr\""),
        Reply::new(304, ""),
      ],
    )
    .expect("rate route");
  fixture
    .route("/graphql", Reply::new(200, graphql_body(false)))
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  engine.github_tick(&tick);
  assert_eq!(engine.world.github_hold_until_ms, 60_000);
  let first = fixture.requests().expect("requests").len();
  for at in [1, 30_000, 59_999, 60_000, 179_999] {
    engine.world.now_ms = at;
    engine.github_tick(&tick);
  }
  assert_eq!(fixture.requests().expect("requests").len(), first);
  for at in [180_000, 360_000] {
    engine.world.now_ms = at;
    engine.github_tick(&tick);
  }
  assert_eq!(engine.world.watches["pr:o/r#1"].next_at_ms, 540_000);
  assert!(engine.world.outbox.iter().any(|step| matches!(step, Step::Journal(row) if row.name == "github-error" && row.note.contains("\"retryAfter\":60"))));
}

#[test]
fn a_rate_limit_rearms_unprocessed_watches_without_moving_their_deadlines() {
  let mut world = World::new(0);
  let key = "pr:Falconiere/toolu#501".to_owned();
  let mut watch = Watch::new(
    Kind::Pr {
      key: "toolu-447".to_owned(),
      repo: "Falconiere/toolu".to_owned(),
      number: 501,
    },
    0,
  );
  assert_eq!(watch.take_due(0), Some(Cause::Immediate));
  let deadline = watch.next_at_ms;
  world.watches.insert(key.clone(), watch);
  rearm(&mut world, &[(key.clone(), Cause::Scheduled)], 0);
  assert_eq!(world.watches.get(&key).expect("watch").next_at_ms, deadline);
  assert_eq!(
    world.watches.get_mut(&key).expect("watch").take_due(1),
    Some(Cause::Immediate)
  );
  assert_eq!(cause_name(Cause::Scheduled), "scheduled");
}
