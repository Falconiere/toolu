use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};

use super::{Attempt, Target, classify, do_merge, merge_method, outcome};
use crate::gate::assess::{Facts, Pr};
use crate::support::gh;

fn pr() -> Pr {
  Pr {
    owner: "o".into(),
    repo: "r".into(),
    number: 7,
  }
}

fn facts(merge_state: &str) -> Facts {
  Facts {
    id: "PR_7".into(),
    merge_state: merge_state.into(),
  }
}

#[test]
fn only_a_protection_refusal_allows_an_admin_retry() {
  for message in [
    "Pull request Falconiere/comemory#1 is not mergeable: the base branch policy prohibits the merge.",
    "To use administrator privileges to immediately merge the pull request, add the `--admin` flag.",
    "At least 1 approving review is required by reviewers with write access.",
  ] {
    assert_eq!(
      outcome(&Attempt::Refused(405, message.into())),
      "admin",
      "{message}"
    );
  }
  for message in [
    "Pull request is not mergeable: merge conflict",
    "Head branch was modified. Review and try the merge again.",
  ] {
    assert_eq!(
      outcome(&Attempt::Refused(409, message.into())),
      "failed",
      "{message}"
    );
  }
  assert_eq!(
    outcome(&Attempt::Uncertain(0, "timeout", "timed out".into())),
    "uncertain"
  );
  assert_eq!(outcome(&Attempt::Merged), "success");
}

/// A fixture where PR 7's merge succeeds and the PR reads merged.
fn merged_fixture() -> Fixture {
  let fixture = Fixture::start().expect("fixture");
  for path in ["/repos/o/r/pulls/7/merge", "/repos/o/r/pulls/7"] {
    fixture
      .route(path, Reply::new(200, r#"{"merged":true}"#))
      .expect("route");
  }
  fixture
}

#[test]
fn a_clean_merge_is_pinned_to_the_head_and_confirmed() {
  let fixture = merged_fixture();
  let client = gh(&fixture);
  let (pr, facts) = (pr(), facts("CLEAN"));
  let target = Target {
    pr: &pr,
    facts: &facts,
    head: "abc123",
    method: "squash",
  };
  let result = do_merge(&client, &target);
  assert_eq!(
    serde_json::Value::Object(result),
    json!({"merged": true, "admin_used": false, "method": "squash"})
  );
  let put = fixture
    .requests()
    .expect("requests")
    .into_iter()
    .find(|request| request.method == "PUT")
    .expect("put");
  assert_eq!(
    serde_json::from_slice::<serde_json::Value>(&put.body).expect("body"),
    json!({"merge_method": "squash", "sha": "abc123"})
  );
}

#[test]
fn a_blocked_pr_merges_with_admin() {
  let fixture = merged_fixture();
  let client = gh(&fixture);
  let pr = pr();
  let blocked = facts("BLOCKED");
  let result = do_merge(
    &client,
    &Target {
      pr: &pr,
      facts: &blocked,
      head: "abc123",
      method: "merge",
    },
  );
  assert_eq!(
    (result["merged"].clone(), result["admin_used"].clone()),
    (json!(true), json!(true))
  );
}

#[test]
fn a_protection_refusal_retries_once_with_admin_and_reports_it() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/repos/o/r/pulls/7/merge",
      Reply::new(
        405,
        r#"{"message":"Required status check \"ci\" is expected."}"#,
      ),
    )
    .expect("refuse");
  let client = gh(&fixture);
  let (pr, clean) = (pr(), facts("CLEAN"));
  let result = do_merge(
    &client,
    &Target {
      pr: &pr,
      facts: &clean,
      head: "abc123",
      method: "merge",
    },
  );
  assert_eq!(
    (result["merged"].clone(), result["admin_used"].clone()),
    (json!(false), json!(true))
  );
  let puts = fixture
    .requests()
    .expect("requests")
    .into_iter()
    .filter(|request| request.method == "PUT")
    .count();
  assert_eq!(puts, 2);
}

#[test]
fn the_merge_method_is_the_repository_s_first_allowed() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/repos/o/r",
      Reply::new(
        200,
        r#"{"allow_squash_merge":false,"allow_merge_commit":true}"#,
      ),
    )
    .expect("repo");
  let client = gh(&fixture);
  assert_eq!(merge_method(&client, &pr(), None).as_deref(), Ok("merge"));
  assert_eq!(
    merge_method(&client, &pr(), Some("rebase")).as_deref(),
    Ok("rebase")
  );
}

#[test]
fn a_moved_head_fails_without_admin() {
  let fixture = Fixture::start().expect("fixture");
  let client = gh(&fixture);
  let pr = pr();
  fixture
    .route(
      "/repos/o/r/pulls/7/merge",
      Reply::new(
        409,
        r#"{"message":"Head branch was modified. Review and try the merge again."}"#,
      ),
    )
    .expect("moved");
  let clean = facts("CLEAN");
  let moved = do_merge(
    &client,
    &Target {
      pr: &pr,
      facts: &clean,
      head: "abc123",
      method: "merge",
    },
  );
  assert_eq!(moved["merged"], json!(false));
  assert!(moved.get("admin_used").is_none());
  assert_eq!(
    (
      moved["native_exit_code"].clone(),
      moved["uncertain"].clone()
    ),
    (json!(409), json!(false))
  );
  assert!(
    moved["error"]
      .as_str()
      .is_some_and(|error| error.contains("Head branch was modified"))
  );
}

#[test]
fn a_timed_out_merge_is_uncertain_and_never_sent_again() {
  let fixture = Fixture::start().expect("fixture");
  let slow = Reply::new(200, r#"{"merged":true}"#).delayed(std::time::Duration::from_secs(3));
  fixture
    .route("/repos/o/r/pulls/7/merge", slow)
    .expect("route");
  let mut config = toolu_github::Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  config.http.timeout = std::time::Duration::from_millis(300);
  let env = crate::support::proxy_env(&fixture).with("GH_TOKEN", "t");
  let client = toolu_github::Client::new(config, &env).expect("client");
  let client = crate::tracker::Gh::with_client(client, crate::support::FAST);
  let (pr, facts) = (pr(), facts("CLEAN"));
  let target = Target {
    pr: &pr,
    facts: &facts,
    head: "abc123",
    method: "squash",
  };
  let result = serde_json::Value::Object(do_merge(&client, &target));
  assert_eq!(
    result,
    json!({"merged": false, "native_exit_code": 0, "native_error_code": "timeout",
      "uncertain": true, "error": "merge command timed out"})
  );
  let puts = fixture.requests().expect("requests");
  assert_eq!(
    puts
      .iter()
      .filter(|request| request.method == "PUT")
      .count(),
    1
  );
}

fn merge_reply(reply: Reply) -> (serde_json::Value, Fixture) {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route("/repos/o/r/pulls/7/merge", reply)
    .expect("route");
  let client = gh(&fixture);
  let (pr, facts) = (pr(), facts("CLEAN"));
  let result = do_merge(
    &client,
    &Target {
      pr: &pr,
      facts: &facts,
      head: "abc123",
      method: "squash",
    },
  );
  (serde_json::Value::Object(result), fixture)
}

#[test]
fn a_server_error_after_a_pinned_merge_is_uncertain_and_not_resent() {
  let (result, fixture) = merge_reply(Reply::new(504, r#"{"message":"Gateway Timeout"}"#));
  assert_eq!(result["merged"], false);
  assert_eq!(result["uncertain"], true);
  assert_eq!(result["native_exit_code"], 504);
  assert_eq!(result["native_error_code"], "http");
  assert!(
    result["error"]
      .as_str()
      .is_some_and(|text| text.contains("504"))
  );
  assert!(result.get("admin_used").is_none());
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0].method, "PUT");
  assert_eq!(
    serde_json::from_slice::<serde_json::Value>(&requests[0].body).expect("body"),
    json!({"merge_method": "squash", "sha": "abc123"})
  );
}

#[test]
fn a_post_write_connection_close_is_uncertain_with_one_connection() {
  let (result, fixture) = merge_reply(Reply::dropped());
  assert_eq!(result["merged"], false);
  assert_eq!(result["uncertain"], true);
  assert_eq!(result["native_error_code"], "transport");
  assert!(result.get("admin_used").is_none());
  assert_eq!(fixture.connects().expect("connects").len(), 1);
}

#[test]
fn local_failures_cannot_trigger_admin_and_response_failures_are_uncertain() {
  let local = classify(toolu_github::Error::Config("protected branch".into()));
  assert_eq!(outcome(&local), "failed");
  assert!(matches!(local, Attempt::Local(_)));
  let local = classify(toolu_github::Error::Transport(
    toolu_http::Error::InvalidConfig("protected branch".into()),
  ));
  assert_eq!(outcome(&local), "failed");
  let no_send = classify(toolu_github::Error::Transport(toolu_http::Error::NotSent(
    "host not found".into(),
  )));
  assert_eq!(outcome(&no_send), "failed");
  let response = classify(toolu_github::Error::Transport(
    toolu_http::Error::BodyTooLarge,
  ));
  assert_eq!(outcome(&response), "uncertain");
}
