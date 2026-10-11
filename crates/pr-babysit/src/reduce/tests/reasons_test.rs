use serde_json::{Value, json};

use super::Facts;

fn facts(checks: &[Value]) -> Facts<'_> {
  Facts {
    pr_state: "OPEN",
    mergeable: "MERGEABLE",
    ci: "pass",
    checks,
    bot_state: "complete",
    bot_verdict: "approved",
    findings: 0,
    degraded: None,
    bot_url: None,
    head: "0123456789abcdef",
    fix_attempts: 0,
    recurring: 0,
    had_rejection: false,
    streak: 0,
    provider_repeated: false,
    unresolved: 0,
    actionable: 0,
    stale: 0,
    fixer: None,
    changed: true,
  }
}

fn codes(reasons: &[Value]) -> Vec<&str> {
  reasons
    .iter()
    .map(|reason| reason["code"].as_str().unwrap())
    .collect()
}

#[test]
fn a_clear_pr_succeeds_and_a_running_fixer_holds_it() {
  let checks = [json!({"name": "ci", "status": "pass"})];
  let clear = facts(&checks);
  assert_eq!(
    codes(&clear.signals()),
    ["ci_pass", "threads_clear", "review_approved"]
  );
  assert_eq!(clear.decision(false), "success");
  assert_eq!(clear.decision(true), "escalate");
  let busy = Facts {
    fixer: Some("fixer running".into()),
    changed: false,
    ..facts(&checks)
  };
  assert_eq!(busy.decision(false), "keep_going");
  assert!(codes(&busy.signals()).ends_with(&["fixer_running", "unchanged"]));
}

#[test]
fn escalations_follow_the_shipped_order() {
  let checks = [];
  let worst = Facts {
    pr_state: "MERGED",
    mergeable: "CONFLICTING",
    fix_attempts: 5,
    recurring: 1,
    had_rejection: true,
    streak: 2,
    provider_repeated: true,
    ..facts(&checks)
  };
  assert_eq!(
    codes(&worst.escalations()),
    [
      "pr_merged",
      "merge_conflict",
      "fix_attempts_exhausted",
      "recurrence_after_rejection",
      "recurrence_streak",
      "provider_error_repeated"
    ]
  );
  assert!(
    worst.escalations()[5]["detail"]
      .as_str()
      .unwrap()
      .ends_with("01234567")
  );
}

#[test]
fn degraded_and_pending_signals_name_their_inputs() {
  let checks = [
    json!({"name": "a", "status": "pending"}),
    json!({"name": "b", "status": "pending"}),
  ];
  let unread = Facts {
    ci: "pending",
    bot_state: "absent",
    degraded: Some("review_absent"),
    mergeable: "UNKNOWN",
    ..facts(&checks)
  };
  let signals = unread.signals();
  assert_eq!(
    codes(&signals),
    [
      "ci_pending",
      "threads_clear",
      "review_absent",
      "manual_verify",
      "mergeable_unknown"
    ]
  );
  assert_eq!(signals[0]["detail"], "a, b");
  assert_eq!(
    signals[3]["detail"],
    "verify review findings manually: no bot comment"
  );
  let none = Facts {
    ci: "pending",
    ..facts(&[])
  };
  assert_eq!(none.signals()[0]["detail"], "no checks reported yet");
}
