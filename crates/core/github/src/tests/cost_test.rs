use serde_json::json;
use toolu_http::Response;

use crate::cost::{Cost, RateLimit};

fn response(status: u16) -> Response {
  Response {
    status,
    headers: [
      ("x-ratelimit-resource", "graphql"),
      ("x-ratelimit-limit", "5000"),
      ("x-ratelimit-remaining", "4997"),
      ("x-ratelimit-used", "3"),
      ("x-ratelimit-reset", "1760000000"),
    ]
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .to_vec(),
    body: Vec::new(),
  }
}

#[test]
fn rest_costs_one_point_and_a_not_modified_none() {
  let rate = RateLimit {
    resource: Some("graphql".into()),
    limit: Some(5000),
    remaining: Some(4997),
    used: Some(3),
    reset: Some(1_760_000_000),
  };
  assert_eq!(
    Cost::rest(&response(200)),
    Cost {
      points: Some(1),
      rate
    }
  );
  assert_eq!(Cost::rest(&response(304)).points, Some(0));
  let bare = Response {
    status: 200,
    headers: Vec::new(),
    body: Vec::new(),
  };
  assert_eq!(Cost::rest(&bare).rate, RateLimit::default());
}

#[test]
fn graphql_cost_is_the_selected_rate_limit_cost() {
  let data = json!({ "rateLimit": { "cost": 3 }, "repository": {} });
  assert_eq!(Cost::graphql(&response(200), &data).points, Some(3));
  assert_eq!(Cost::graphql(&response(200), &json!({})).points, None);
  assert_eq!(
    serde_json::to_value(Cost::graphql(&response(200), &data)).expect("json"),
    json!({
      "points": 3,
      "rate": { "resource": "graphql", "limit": 5000, "remaining": 4997, "used": 3,
                "reset": 1_760_000_000 }
    })
  );
}
