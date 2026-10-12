use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::JiraClient;
use crate::support::{FAST, http_client, jira_env};
use crate::tracker::rest::Http;

fn client(fixture: &Fixture, env: &Env) -> Result<JiraClient, String> {
  JiraClient::with_http(
    env,
    Http::with_client(http_client(fixture), FAST, "epic jira"),
  )
}

fn issue(key: &str) -> serde_json::Value {
  json!({"key": key, "fields": {"summary": key, "labels": ["a"]}})
}

#[test]
fn missing_base_or_credentials_name_every_variable_before_any_request() {
  let fixture = Fixture::start().expect("fixture");
  let without_base = Env::from_pairs([("JIRA_PAT", "p")]);
  let without_creds = Env::from_pairs([("JIRA_BASE_URL", "https://x")]);
  for env in [without_base, without_creds] {
    let err = client(&fixture, &env).err().expect("refused");
    for name in ["JIRA_BASE_URL", "JIRA_PAT", "JIRA_EMAIL", "JIRA_API_TOKEN"] {
      assert!(err.contains(name), "{err}");
    }
  }
  let ftp = Env::from_pairs([("JIRA_BASE_URL", "ftp://x"), ("JIRA_PAT", "p")]);
  assert_eq!(
    client(&fixture, &ftp).err().as_deref(),
    Some("JIRA_BASE_URL must use http or https")
  );
  let v4 = jira_env(&fixture, "4");
  assert_eq!(
    client(&fixture, &v4).err().as_deref(),
    Some("JIRA_API_VERSION must be 2 or 3")
  );
  assert_eq!(fixture.requests().expect("requests").len(), 0);
}

#[test]
fn v3_search_follows_next_page_tokens_with_bearer_auth() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/rest/api/3/search/jql",
      vec![
        Reply::new(
          200,
          json!({"issues": [issue("PAY-12")], "nextPageToken": "next"}).to_string(),
        ),
        Reply::new(200, json!({"issues": [issue("PAY-13")]}).to_string()),
      ],
    )
    .expect("route");
  let jira = client(&fixture, &jira_env(&fixture, "3")).expect("client");
  let found = jira.search("parent = PAY-7 ORDER BY key").expect("search");
  assert_eq!(found.len(), 2);
  let requests = fixture.requests().expect("requests");
  assert_eq!(
    requests[0].headers.get("authorization").map(String::as_str),
    Some("Bearer pat-token")
  );
  let second: serde_json::Value = serde_json::from_slice(&requests[1].body).expect("json");
  assert_eq!(second["nextPageToken"], "next");
  assert_eq!(second["jql"], "parent = PAY-7 ORDER BY key");
}

#[test]
fn v2_basic_search_pages_by_total_and_comments_as_plain_text() {
  let fixture = Fixture::start().expect("fixture");
  let env = proxy_env_v2(&fixture);
  fixture
    .sequence(
      "/rest/api/2/search",
      vec![
        Reply::new(
          200,
          json!({"issues": [issue("PAY-12")], "total": 2}).to_string(),
        ),
        Reply::new(
          200,
          json!({"issues": [issue("PAY-13")], "total": 2}).to_string(),
        ),
      ],
    )
    .expect("route");
  fixture
    .route("/rest/api/2/issue/PAY-12/comment", Reply::new(201, "{}"))
    .expect("route");
  let jira = client(&fixture, &env).expect("client");
  assert_eq!(jira.search("x").expect("search").len(), 2);
  jira.comment("PAY-12", "Delivered").expect("comment");
  let requests = fixture.requests().expect("requests");
  let second: serde_json::Value = serde_json::from_slice(&requests[1].body).expect("json");
  assert_eq!(second["startAt"], 1);
  let comment: serde_json::Value = serde_json::from_slice(&requests[2].body).expect("json");
  assert_eq!(comment, json!({"body": "Delivered"}));
  let basic = requests[0]
    .headers
    .get("authorization")
    .cloned()
    .unwrap_or_default();
  assert_eq!(basic, "Basic YWdlbnRAZXhhbXBsZS5jb206YXBpLXRva2Vu");
}

fn proxy_env_v2(fixture: &Fixture) -> Env {
  crate::support::proxy_env(fixture)
    .with("JIRA_BASE_URL", &fixture.url(""))
    .with("JIRA_EMAIL", "agent@example.com")
    .with("JIRA_API_TOKEN", "api-token")
    .with("JIRA_API_VERSION", "2")
}

#[test]
fn transition_requires_exactly_one_named_match() {
  let fixture = Fixture::start().expect("fixture");
  let rows = json!({"transitions": [{"id": "1", "name": "Done"}, {"id": "2", "name": "done"}]});
  fixture
    .route(
      "/rest/api/3/issue/PAY-1/transitions",
      Reply::new(200, rows.to_string()),
    )
    .expect("route");
  let jira = client(&fixture, &jira_env(&fixture, "3")).expect("client");
  let err = jira.transition("PAY-1", "DONE").expect_err("ambiguous");
  assert_eq!(err, "Jira transition \"DONE\" matched 2 transitions");
  assert!(jira.issue("PAY-1", &[]).is_err());
}

#[test]
fn malformed_replies_are_refused() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/rest/api/3/issue/PAY-1", Reply::new(200, r#"{"key":1}"#))
    .expect("route");
  fixture
    .route(
      "/rest/api/3/issue/PAY-1/transitions",
      Reply::new(200, r#"{"transitions":[{"id":1}]}"#),
    )
    .expect("route");
  fixture
    .route(
      "/rest/api/3/search/jql",
      Reply::new(200, r#"{"issues":[{"key":"A"}]}"#),
    )
    .expect("route");
  let jira = client(&fixture, &jira_env(&fixture, "3")).expect("client");
  assert_eq!(
    jira.issue("PAY-1", &[]).err().as_deref(),
    Some("invalid Jira issue response")
  );
  assert_eq!(
    jira.transitions("PAY-1").err().as_deref(),
    Some("invalid Jira transitions response")
  );
  assert_eq!(
    jira.search("x").err().as_deref(),
    Some("invalid Jira search response")
  );
  assert!(jira.raw_issue("PAY-1").is_ok());
}
