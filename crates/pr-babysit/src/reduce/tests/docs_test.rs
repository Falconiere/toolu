use serde_json::json;

use super::comparison;
use crate::reduce::inputs::bot;

#[test]
fn the_comparison_lists_the_eight_change_fields() {
  let bot = bot(&json!({}), None);
  let fields = comparison(&json!({"mergeable": "MERGEABLE"}), "pass", 2, "abc", &bot);
  let keys: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
  assert_eq!(
    keys,
    [
      "ciStatus",
      "reviewDecision",
      "mergeable",
      "unresolvedThreads",
      "headSha",
      "botVerdict",
      "botState",
      "botFindingKeys"
    ]
  );
  assert_eq!(fields[1].1, serde_json::Value::Null);
  assert_eq!(fields[2].1, "MERGEABLE");
  assert_eq!(fields[3].1, 2);
  assert_eq!(fields[6].1, "absent");
}

fn submission(machine: Option<&str>) -> serde_json::Value {
  serde_json::to_value(toolu_mux::relay::store::Submission {
    key: "fixer-1".into(),
    origin: "https://example.test/owner/repo.git".into(),
    base: "0123456789abcdef0123456789abcdef01234567".into(),
    brief: "Fix the review finding".into(),
    agent: "codex".into(),
    argv: vec!["codex".into()],
    machine: machine.map(str::to_owned),
    group: Some("owner/repo#165".into()),
  })
  .unwrap()
}

#[test]
fn configured_relay_routing_survives_repeated_ticks() {
  use crate::reduce::tests::{LATER, NOW, open_165, run};

  let input = open_165();
  for (enabled, machine) in [
    (true, json!(null)),
    (true, json!("worker-1")),
    (false, json!("worker-1")),
  ] {
    let mut previous = run(&input, None, NOW).state;
    previous["relayEnabled"] = json!(enabled);
    previous["relayMachine"] = machine.clone();
    for _ in 0..2 {
      previous = run(&input, Some(&previous), LATER).state;
      assert_eq!(previous.get("relayEnabled"), Some(&json!(enabled)));
      assert_eq!(previous.get("relayMachine"), Some(&machine));
    }
  }
}

#[test]
fn complete_persisted_submission_recovers_relay_routing() {
  use crate::reduce::tests::{LATER, NOW, open_165, run};

  let input = open_165();
  for machine in [None, Some("worker-1")] {
    let request = submission(machine);
    let mut previous = run(&input, None, NOW).state;
    previous["fixer"] = json!({"status": "running", "current": 1, "groups": [{"relayRequest": request}], "items": []});
    for _ in 0..2 {
      previous = run(&input, Some(&previous), LATER).state;
      assert_eq!(previous.get("relayEnabled"), Some(&json!(true)));
      assert_eq!(previous.get("relayMachine"), Some(&json!(machine)));
      assert_eq!(previous["fixer"]["groups"][0]["relayRequest"], request);
    }
  }
}

#[test]
fn invalid_or_absent_submission_does_not_infer_relay_and_explicit_false_wins() {
  use crate::reduce::tests::{LATER, NOW, open_165, run};

  let input = open_165();
  for request in [json!(null), json!({"key": "incomplete"}), json!("invalid")] {
    let mut previous = run(&input, None, NOW).state;
    previous["fixer"] = json!({"status": "running", "current": 1, "groups": [{"relayRequest": request}], "items": []});
    for _ in 0..2 {
      previous = run(&input, Some(&previous), LATER).state;
      assert_eq!(previous.get("relayEnabled"), None);
      assert_eq!(previous.get("relayMachine"), None);
    }
  }
  let mut previous = run(&input, None, NOW).state;
  previous["relayEnabled"] = json!(false);
  previous["fixer"] = json!({"status": "running", "current": 1, "groups": [{"relayRequest": submission(Some("worker-1"))}], "items": []});
  for _ in 0..2 {
    previous = run(&input, Some(&previous), LATER).state;
    assert_eq!(previous.get("relayEnabled"), Some(&json!(false)));
    assert_eq!(previous.get("relayMachine"), None);
  }
}
