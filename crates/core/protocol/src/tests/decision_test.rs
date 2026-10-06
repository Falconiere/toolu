//! The cases of `decision.test.ts` and `policy.test.ts`, plus the strict wire.

use serde_json::json;

use super::{Decision, FailureCode, merge};
use crate::text::Text;

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn parse(value: serde_json::Value) -> Result<Decision, String> {
  serde_json::from_value(value).map_err(|err| err.to_string())
}

#[test]
fn every_kind_parses_from_the_typescript_wire() {
  assert_eq!(parse(json!({ "kind": "allow" })), Ok(Decision::Allow));
  assert_eq!(
    parse(json!({ "kind": "deny", "reason": "blocked" })),
    Ok(Decision::Deny {
      reason: text("blocked")
    })
  );
  assert_eq!(
    parse(json!({ "kind": "ask", "reason": "confirm" })),
    Ok(Decision::Ask {
      reason: text("confirm")
    })
  );
  assert_eq!(
    parse(json!({ "kind": "advisory", "message": "note" })),
    Ok(Decision::Advisory {
      message: text("note")
    })
  );
  assert_eq!(
    parse(json!({ "kind": "post_block", "reason": "lint failed" })),
    Ok(Decision::Block {
      reason: text("lint failed")
    })
  );
  assert_eq!(
    parse(json!({ "kind": "runtime_failure", "reason": "timeout", "code": "timeout" })),
    Ok(Decision::RuntimeFailure {
      reason: text("timeout"),
      code: FailureCode::Timeout
    })
  );
}

#[test]
fn every_failure_code_parses() {
  for (name, code) in [
    ("timeout", FailureCode::Timeout),
    ("spawn", FailureCode::Spawn),
    ("parse", FailureCode::Parse),
    ("truncated", FailureCode::Truncated),
    ("cancelled", FailureCode::Cancelled),
    ("nonzero", FailureCode::Nonzero),
  ] {
    let decision = parse(json!({ "kind": "runtime_failure", "reason": "r", "code": name }));
    assert_eq!(
      decision,
      Ok(Decision::RuntimeFailure {
        reason: text("r"),
        code
      })
    );
  }
}

#[test]
fn every_decision_writes_the_typescript_wire_and_reads_back() {
  let decisions = [
    (Decision::Allow, r#"{"kind":"allow"}"#),
    (
      Decision::Block {
        reason: text("lint failed"),
      },
      r#"{"kind":"post_block","reason":"lint failed"}"#,
    ),
    (
      Decision::Advisory {
        message: text("run the tests"),
      },
      r#"{"kind":"advisory","message":"run the tests"}"#,
    ),
    (
      Decision::RuntimeFailure {
        reason: text("gate crashed"),
        code: FailureCode::Nonzero,
      },
      r#"{"kind":"runtime_failure","reason":"gate crashed","code":"nonzero"}"#,
    ),
  ];
  for (decision, wire) in decisions {
    assert_eq!(serde_json::to_string(&decision).unwrap(), wire);
    assert_eq!(serde_json::from_str::<Decision>(wire).unwrap(), decision);
  }
}

#[test]
fn an_unknown_kind_or_code_is_refused() {
  assert!(parse(json!({ "kind": "maybe" })).is_err());
  assert!(parse(json!({ "kind": "runtime_failure", "reason": "r", "code": "oom" })).is_err());
  assert!(parse(json!({ "reason": "r" })).is_err());
}

#[test]
fn a_missing_or_empty_required_field_is_refused() {
  let err = parse(json!({ "kind": "deny" })).unwrap_err();
  assert!(
    err.contains("a `deny` decision takes exactly `reason`"),
    "{err}"
  );
  assert!(parse(json!({ "kind": "deny", "reason": "" })).is_err());
  assert!(parse(json!({ "kind": "advisory", "message": "" })).is_err());
  assert!(parse(json!({ "kind": "runtime_failure", "reason": "r" })).is_err());
}

#[test]
fn an_unknown_field_is_refused_even_on_allow() {
  let err = parse(json!({ "kind": "allow", "x": 1 })).unwrap_err();
  assert!(err.contains("unknown field `x`"), "{err}");
  assert!(parse(json!({ "kind": "deny", "reason": "r", "x": 1 })).is_err());
}

#[test]
fn a_field_of_another_kind_is_refused() {
  let err = parse(json!({ "kind": "allow", "reason": "r" })).unwrap_err();
  assert!(err.contains("an `allow` decision takes no fields"), "{err}");
  assert!(parse(json!({ "kind": "deny", "message": "m" })).is_err());
  assert!(parse(json!({ "kind": "advisory", "message": "m", "reason": "r" })).is_err());
  assert!(parse(json!({ "kind": "ask", "reason": "r", "code": "spawn" })).is_err());
}

#[test]
fn deny_beats_ask_beats_advisory_beats_allow() {
  let allow = Decision::Allow;
  let advisory = Decision::Advisory {
    message: text("note"),
  };
  let ask = Decision::Ask {
    reason: text("prompt"),
  };
  let deny = Decision::Deny { reason: text("no") };
  let all = [allow.clone(), advisory.clone(), ask.clone(), deny.clone()];
  assert_eq!(merge(&all), deny);
  assert_eq!(merge(&[allow.clone(), advisory.clone(), ask.clone()]), ask);
  assert_eq!(merge(&[allow, advisory.clone()]), advisory);
}

#[test]
fn an_empty_list_merges_to_allow() {
  assert_eq!(merge(&[]), Decision::Allow);
}

#[test]
fn a_runtime_failure_beats_a_deny() {
  let failure = Decision::RuntimeFailure {
    reason: text("crashed"),
    code: FailureCode::Spawn,
  };
  let deny = Decision::Deny { reason: text("no") };
  assert_eq!(merge(&[deny, failure.clone()]), failure);
}

#[test]
fn deny_and_post_block_tie_so_the_first_wins() {
  let deny = Decision::Deny {
    reason: text("first"),
  };
  let block = Decision::Block {
    reason: text("second"),
  };
  assert_eq!(merge(&[deny.clone(), block.clone()]), deny);
  assert_eq!(merge(&[block.clone(), deny]), block);
}
