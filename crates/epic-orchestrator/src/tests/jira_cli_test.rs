use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{USAGE, issue_get, parse_key};
use crate::support::{FAST, http_client, jira_env};
use crate::tracker::jira_client::JiraClient;
use crate::tracker::rest::Http;

#[test]
fn an_issue_prints_as_pretty_json() {
  let fixture = Fixture::start().expect("fixture");
  let issue = json!({"key": "PAY-12", "fields": {"summary": "Refund flow"}});
  fixture
    .route(
      "/rest/api/3/issue/PAY-12",
      Reply::new(200, issue.to_string()),
    )
    .expect("route");
  let http = Http::with_client(http_client(&fixture), FAST, "epic jira");
  let client = JiraClient::with_http(&jira_env(&fixture, "3"), http).expect("client");
  let text = issue_get(&client, &parse_key("PAY-12").expect("key")).expect("issue");
  assert_eq!(
    text,
    format!(
      "{}\n",
      serde_json::to_string_pretty(&issue).expect("pretty")
    )
  );
}

#[test]
fn bad_keys_are_refused_before_any_request() {
  for raw in ["", "nope", "PAY-", "12-PAY"] {
    assert_eq!(parse_key(raw), Err(USAGE.to_owned()), "{raw}");
  }
  assert_eq!(parse_key("jira:pay-12").as_deref(), Ok("PAY-12"));
}

#[test]
fn missing_credentials_are_refused_before_any_request() {
  let env = toolu_runtime::env::Env::from_pairs([("HOME", "/tmp")]);
  let err = JiraClient::new(&env).err().expect("no credentials");
  assert!(err.contains("JIRA_BASE_URL"), "{err}");
}
