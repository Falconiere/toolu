use serde_json::{Map, Value, json};
use toolu_http_test_support::{Fixture, Reply};

use super::{Action, action, arm, disarm, state};
use crate::gate::assess::Facts;
use crate::support::gh;

fn map(value: &Value) -> Map<String, Value> {
  value.as_object().cloned().expect("object")
}

#[test]
fn auto_merge_arms_only_on_wait_and_disarms_before_a_push() {
  assert_eq!(action("wait", false, true), Some(Action::Arm));
  assert_eq!(action("wait", false, false), None);
  assert_eq!(action("wait", true, true), None);
  assert_eq!(action("merge", false, true), None);
  assert_eq!(action("rebase", true, false), Some(Action::Disarm));
  assert_eq!(action("fix", true, true), Some(Action::Disarm));
  assert_eq!(action("fix", false, true), None);
}

#[test]
fn the_recorded_state_reflects_this_run() {
  assert_eq!(
    state(&map(
      &json!({"auto_merge_armed": true, "auto_merge_disarmed": true})
    )),
    "off"
  );
  assert_eq!(
    state(&map(
      &json!({"auto_merge_armed": false, "auto_merge": true})
    )),
    "armed"
  );
  assert_eq!(
    state(&map(
      &json!({"auto_merge_armed": false, "auto_merge": false})
    )),
    "unavailable"
  );
  assert_eq!(state(&map(&json!({"auto_merge_armed": true}))), "armed");
  assert_eq!(state(&map(&json!({"auto_merge_armed": false}))), "off");
}

#[test]
fn arming_is_pinned_to_the_head_and_a_refusal_is_recorded() {
  let fixture = Fixture::start().expect("fixture");
  fixture
    .route(
      "/graphql",
      Reply::new(
        200,
        r#"{"data":{"enablePullRequestAutoMerge":{"clientMutationId":null}}}"#,
      ),
    )
    .expect("arm");
  let client = gh(&fixture);
  let facts = Facts {
    id: "PR_7".into(),
    merge_state: "BLOCKED".into(),
  };
  assert_eq!(
    Value::Object(arm(&client, &facts, "abc123", "squash")),
    json!({"auto_merge": true, "method": "squash"})
  );
  let sent: Value =
    serde_json::from_slice(&fixture.requests().expect("requests")[0].body).expect("body");
  assert_eq!(
    sent["variables"],
    json!({"id": "PR_7", "m": "SQUASH", "h": "abc123"})
  );
  fixture
    .route(
      "/graphql",
      Reply::new(
        200,
        r#"{"errors":[{"message":"Auto merge is not allowed for this repository"}]}"#,
      ),
    )
    .expect("refuse");
  let refused = arm(&client, &facts, "abc123", "squash");
  assert_eq!(refused["auto_merge"], json!(false));
  assert!(
    refused["auto_merge_error"]
      .as_str()
      .is_some_and(|err| err.contains("Auto merge is not allowed"))
  );
}

#[test]
fn disarming_clears_auto_merge() {
  let fixture = Fixture::start().expect("fixture");
  let body = r#"{"data":{"disablePullRequestAutoMerge":{"clientMutationId":null}}}"#;
  fixture
    .route("/graphql", Reply::new(200, body))
    .expect("disarm");
  let facts = Facts {
    id: "PR_7".into(),
    merge_state: "CLEAN".into(),
  };
  assert_eq!(disarm(&gh(&fixture), &facts), Ok(()));
  let sent: Value =
    serde_json::from_slice(&fixture.requests().expect("requests")[0].body).expect("body");
  assert_eq!(sent["variables"], json!({"id": "PR_7"}));
}
