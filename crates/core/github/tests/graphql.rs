//! GraphQL queries against the loopback API: the reported cost, `errors[]`
//! and a reply without data.

#[path = "helpers/api.rs"]
mod api;

use api::Api;
use serde_json::{Value, json};
use toolu_github::{Config, Error};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

const QUERY: &str = "query($n: Int!) { rateLimit { cost remaining resetAt } \
  repository(owner: \"o\", name: \"r\") { pullRequest(number: $n) { headRefOid } } }";

fn env() -> Env {
  Env::from_pairs([("GH_TOKEN", "graphql-token"), ("PB_GH_BACKOFF", "0 0 0")])
}

#[test]
fn a_query_selecting_rate_limit_reports_its_cost() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  let body = json!({ "data": {
    "rateLimit": { "cost": 3, "remaining": 4997, "resetAt": "2026-10-07T05:00:00Z" },
    "repository": { "pullRequest": { "headRefOid": "abc123" } }
  }});
  api
    .fixture
    .route(
      "/graphql",
      Reply::new(200, body.to_string())
        .header("X-RateLimit-Resource", "graphql")
        .header("X-RateLimit-Used", "3"),
    )
    .expect("route");
  let reply = client.graphql(QUERY, &json!({ "n": 460 })).expect("reply");
  assert_eq!(
    reply.data["repository"]["pullRequest"]["headRefOid"],
    "abc123"
  );
  assert_eq!(reply.cost.points, Some(3));
  assert_eq!(reply.cost.rate.resource.as_deref(), Some("graphql"));
  assert_eq!(reply.cost.rate.used, Some(3));
  let requests = api.fixture.requests().expect("requests");
  assert_eq!(
    (requests[0].method.as_str(), requests[0].path.as_str()),
    ("POST", "/graphql")
  );
  assert_eq!(
    serde_json::from_slice::<Value>(&requests[0].body).expect("body"),
    json!({ "query": QUERY, "variables": { "n": 460 } })
  );
}

#[test]
fn graphql_errors_fail_after_one_request() {
  let api = Api::start().expect("api");
  let client = api
    .client(Config::one_shot(&env()).expect("config"), &env())
    .expect("client");
  let body =
    json!({ "data": null, "errors": [{ "type": "NOT_FOUND", "message": "Could not resolve" }] });
  api
    .fixture
    .route("/graphql", Reply::new(200, body.to_string()))
    .expect("route");
  assert_eq!(
    client
      .graphql(QUERY, &json!({ "n": 1 }))
      .map(|reply| reply.data),
    Err(Error::GraphQl(vec!["Could not resolve".into()]))
  );
  assert_eq!(api.fixture.requests().expect("requests").len(), 1);
}

#[test]
fn a_reply_without_data_cannot_be_decoded() {
  let api = Api::start().expect("api");
  let client = api.client(Config::scheduled(), &env()).expect("client");
  api
    .fixture
    .route("/graphql", Reply::new(200, "{}"))
    .expect("route");
  assert!(matches!(
    client.graphql(QUERY, &json!({})),
    Err(Error::Decode(_))
  ));
}
