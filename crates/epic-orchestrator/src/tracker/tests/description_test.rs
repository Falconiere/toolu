use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use crate::support::{FAST, gh, http_client, jira_env, proxy_env};
use crate::tracker::jira_client::JiraClient;
use crate::tracker::linear::LinearTracker;
use crate::tracker::rest::Http;

#[test]
fn github_fetches_full_description_and_refuses_missing_body() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/repos/O/R/issues/2",
      vec![
        Reply::new(200, json!({"body":"full\nbody"}).to_string()),
        Reply::new(200, json!({"title":"missing"}).to_string()),
      ],
    )
    .expect("route");
  let client = gh(&fixture);
  assert_eq!(super::github(&client, "O/R#2").expect("body"), "full\nbody");
  assert!(super::github(&client, "O/R#2").is_err());
}

fn jira_description(body: &serde_json::Value) -> Result<String, String> {
  let fixture = Fixture::start().map_err(|error| format!("{error:?}"))?;
  fixture
    .route(
      "/rest/api/3/issue/PAY-2?fields=description",
      Reply::new(200, body.to_string()),
    )
    .map_err(|error| format!("{error:?}"))?;
  let http = Http::with_client(http_client(&fixture), FAST, "epic jira");
  let client = JiraClient::with_http(&jira_env(&fixture, "3"), http)?;
  super::jira(&client, "PAY-2")
}

#[test]
fn jira_fetches_adf_and_refuses_malformed_description() {
  let adf = json!({"key":"PAY-2","fields":{"description":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"full Jira task"}]}]}}});
  assert!(
    jira_description(&adf)
      .expect("body")
      .contains("full Jira task")
  );
  assert!(jira_description(&json!({"key":"PAY-2","fields":{"description":123}})).is_err());
}

#[test]
fn linear_fetches_full_description_and_refuses_missing_issue() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .sequence(
      "/graphql",
      vec![
        Reply::new(
          200,
          json!({"data":{"issue":{"description":"full Linear task"}}}).to_string(),
        ),
        Reply::new(200, json!({"data":{"issue":null}}).to_string()),
      ],
    )
    .expect("route");
  let env = proxy_env(&fixture)
    .with("LINEAR_API_KEY", "fixture")
    .with("LINEAR_API_URL", &fixture.url("/graphql"));
  let http = Http::with_client(http_client(&fixture), FAST, "Linear");
  let client = LinearTracker::with("ENG-2", "a/b", &env, http).expect("client");
  assert_eq!(
    super::linear(&client, "ENG-2").expect("body"),
    "full Linear task"
  );
  assert!(super::linear(&client, "ENG-2").is_err());
}
