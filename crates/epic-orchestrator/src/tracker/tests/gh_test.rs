use serde_json::json;
use toolu_github::Method;
use toolu_http_test_support::{Fixture, Reply};

use super::next_link;
use crate::support::{gh, ok};

#[test]
fn get_all_follows_next_links_across_pages() {
  let fixture = Fixture::start().expect("fixture");
  let next = fixture.url("/items?page=2");
  fixture
    .route(
      "/items?page=1",
      Reply::new(200, "[1,2]").header("Link", &format!("<{next}>; rel=\"next\"")),
    )
    .expect("route");
  ok(&fixture, "/items?page=2", &json!([3]));
  let items = gh(&fixture).get_all("/items?page=1").expect("pages");
  assert_eq!(items, vec![json!(1), json!(2), json!(3)]);
}

#[test]
fn a_write_is_not_repeated_after_a_5xx_but_a_read_is() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/repos/o/r/issues/1",
      Reply::new(502, "{\"message\":\"bad gateway\"}"),
    )
    .expect("route");
  let client = gh(&fixture);
  let err = client
    .write(
      Method::Patch,
      "/repos/o/r/issues/1",
      &json!({"state": "closed"}),
    )
    .expect_err("write");
  assert!(err.contains("502"), "{err}");
  assert_eq!(fixture.requests().expect("requests").len(), 1);
  let read = client.get::<serde_json::Value>("/repos/o/r/issues/1");
  let read = read.expect_err("read");
  assert!(read.contains("502"), "{read}");
  assert_eq!(fixture.requests().expect("requests").len(), 3);
}

#[test]
fn graphql_returns_data_and_refuses_errors() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/graphql",
      vec![
        Reply::new(200, r#"{"data":{"viewer":{"login":"a"}}}"#),
        Reply::new(200, r#"{"errors":[{"message":"no such field"}]}"#),
      ],
    )
    .expect("route");
  let client = gh(&fixture);
  let data = client
    .graphql("{viewer{login}}", &json!({}), false)
    .expect("data");
  assert_eq!(data["viewer"]["login"], "a");
  let err = client
    .graphql("{nope}", &json!({}), false)
    .expect_err("errors");
  assert!(err.contains("no such field"), "{err}");
}

#[test]
fn next_link_reads_only_the_next_relation() {
  let headers = vec![(
    "link".to_owned(),
    "<https://x/a?page=1>; rel=\"prev\", <https://x/a?page=3>; rel=\"next\"".to_owned(),
  )];
  assert_eq!(next_link(&headers).as_deref(), Some("https://x/a?page=3"));
  assert_eq!(next_link(&[]), None);
}
