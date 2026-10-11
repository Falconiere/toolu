use serde_json::json;
use toolu_runtime::env::Env;

use super::*;
use crate::fixture;

fn items() -> Value {
  fixture::json("items/review-items.json")
}

fn first_id() -> String {
  items()["items"][0]["id"].as_str().unwrap().to_owned()
}

fn group(seq: &Value, host: &str) -> Value {
  json!({"seq": seq, "tier": "standard", "host": host, "model": "gpt-6-sol", "effort": "medium", "items": [first_id()]})
}

fn refused(plan: &Value, env: &Env) -> (String, String) {
  let state = json!({"version": 2, "repo": "Falconiere/toolu", "number": 165, "slot": "falconiere-toolu-165"});
  let failure = validate(plan, &items(), &state, env).unwrap_err();
  (failure.code.to_owned(), failure.message)
}

fn message(plan: &Value) -> String {
  refused(plan, &Env::from_pairs([("PATH", "/usr/bin:/bin")])).1
}

#[test]
fn a_well_formed_plan_passes() {
  let plan =
    json!({"dispatch": "herdr", "groups": [group(&json!(1), "codex"), group(&json!(2), "claude")]});
  let state = json!({"slot": "falconiere-toolu-165"});
  assert!(
    validate(
      &plan,
      &items(),
      &state,
      &Env::from_pairs([("PATH", "/usr/bin")])
    )
    .is_ok()
  );
}

#[test]
fn malformed_plan_groups_are_refused_with_the_typescript_messages() {
  let needs = "every plan group needs a host and string item ids";
  assert_eq!(
    message(&json!({"dispatch": "herdr", "groups": [null]})),
    needs
  );
  let gap =
    json!({"dispatch": "herdr", "groups": [group(&json!(1), "codex"), group(&json!(3), "codex")]});
  assert_eq!(
    message(&gap),
    "plan groups need contiguous 1-based seq values"
  );
  for seq in [json!("2"), json!(2.5), Value::Null] {
    let plan =
      json!({"dispatch": "herdr", "groups": [group(&json!(1), "codex"), group(&seq, "codex")]});
    assert_eq!(
      message(&plan),
      "every plan group needs a positive integer seq"
    );
  }
  assert_eq!(
    message(&json!({"dispatch": "inline", "groups": []})),
    "the plan dispatches inline; run this round in-session"
  );
  assert_eq!(
    message(&json!({"dispatch": "herdr", "groups": []})),
    "the plan has no groups"
  );
  let mut unknown = group(&json!(1), "codex");
  unknown["items"] = json!(["thread:nope"]);
  assert_eq!(
    message(&json!({"dispatch": "herdr", "groups": [unknown]})),
    "the plan names an item that is not in the items file"
  );
}

#[test]
fn hosts_routing_and_opencode_config_are_checked_before_any_worktree() {
  let env = Env::from_pairs([("PATH", "/usr/bin")]);
  let (code, text) = refused(
    &json!({"dispatch": "herdr", "groups": [group(&json!(1), "vim")]}),
    &env,
  );
  assert_eq!(code, "config_invalid");
  assert_eq!(
    text,
    "plan group 1 names host 'vim'; use claude, codex, cursor or opencode"
  );
  let mut unsafe_model = group(&json!(1), "codex");
  unsafe_model["model"] = json!("gpt; rm -rf /");
  let (code, _) = refused(
    &json!({"dispatch": "herdr", "groups": [unsafe_model]}),
    &env,
  );
  assert_eq!(code, "config_invalid");
  let bad = env.with("OPENCODE_CONFIG_CONTENT", "[1]");
  let (code, text) = refused(
    &json!({"dispatch": "herdr", "groups": [group(&json!(1), "opencode")]}),
    &bad,
  );
  assert_eq!(code, "config_invalid");
  assert!(text.contains("OPENCODE_CONFIG_CONTENT"), "{text}");
}

#[test]
fn read_object_refuses_a_missing_or_non_object_file() {
  let dir = tempfile::tempdir().unwrap();
  let list = dir.path().join("list.json");
  std::fs::write(&list, "[]").unwrap();
  for path in [dir.path().join("missing.json"), list] {
    let failure = read_object(&path, "plan_invalid", "plan is missing".into()).unwrap_err();
    assert_eq!(
      (failure.code, failure.message.as_str()),
      ("plan_invalid", "plan is missing")
    );
  }
  assert!(
    read_object(
      &fixture::path("items/review-items.json"),
      "x",
      String::new()
    )
    .is_ok()
  );
}

#[test]
fn a_bad_round_or_an_injection_flagged_item_is_refused() {
  let env = Env::from_pairs([("PATH", "/usr/bin")]);
  let plan = json!({"dispatch": "herdr", "groups": [group(&json!(1), "codex")]});
  let mut zero = items();
  zero["round"] = json!(0);
  let failure = validate(&plan, &zero, &json!({}), &env).unwrap_err();
  assert_eq!(failure.message, "round must be a positive integer");
  let flagged = json!({"actions": {"flagged": {first_id(): {"at": "2026-10-03T12:00:00Z"}}}});
  let failure = validate(&plan, &items(), &flagged, &env).unwrap_err();
  assert_eq!(
    failure.message,
    "the plan includes an injection-flagged thread"
  );
}
