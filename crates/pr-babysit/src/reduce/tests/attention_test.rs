use serde_json::{Value, json};

use super::Work;
use crate::fixture;
use crate::reduce::Reduced;
use crate::reduce::tests::{LATER, NOW, open_165, open_ci_thread, run};

fn work<'a>(threads: &'a [Value], escalations: &'a [Value]) -> Work<'a> {
  Work {
    escalations,
    ci: "pass",
    checks: &[],
    head: "abc",
    threads,
    stale: &[],
    conversation: &[],
    reviews: &[],
    review_changes: false,
    provider_error: false,
    bot_run: "1@t".into(),
    fixer: None,
  }
}

#[test]
fn the_fingerprint_follows_the_reply_target_not_the_tick() {
  let one = [json!({"id": "PRRT_a", "inReplyTo": 7})];
  let first = work(&one, &[]).attention(None);
  assert_eq!(first["codes"], json!(["threads_actionable"]));
  let print = first["fingerprint"].as_str().unwrap();
  assert_eq!(work(&one, &[]).attention(Some(print))["new"], false);
  let follow_up = [json!({"id": "PRRT_a", "inReplyTo": 8})];
  let next = work(&follow_up, &[]).attention(Some(print));
  assert_eq!(next["new"], true);
  assert_ne!(next["fingerprint"], first["fingerprint"]);
}

#[test]
fn nothing_to_do_has_no_fingerprint_and_escalations_come_first() {
  let none = work(&[], &[]).attention(Some("old"));
  assert_eq!(
    none,
    json!({"needed": false, "new": false, "codes": [], "fingerprint": null})
  );
  let escalations = [json!({"code": "merge_conflict", "detail": "x"})];
  let threads = [json!({"id": "t", "inReplyTo": 1})];
  let both = work(&threads, &escalations).attention(None);
  assert_eq!(both["codes"], json!(["escalate", "threads_actionable"]));
}

fn attention_codes(reduced: &Reduced) -> Vec<&str> {
  reduced.result["attention"]["codes"]
    .as_array()
    .unwrap()
    .iter()
    .map(|code| code.as_str().unwrap())
    .collect()
}

#[test]
fn a_green_approved_pr_needs_no_attention() {
  let reduced = run(&open_165(), None, NOW);
  assert_eq!(reduced.result["decision"], "success");
  assert_eq!(
    reduced.result["attention"],
    json!({"needed": false, "new": false, "codes": [], "fingerprint": null})
  );
  assert_eq!(reduced.state["attention"], json!({"fingerprint": null}));
}

#[test]
fn one_open_thread_is_new_attention_once() {
  let mut input = open_165();
  open_ci_thread(&mut input);
  let first = run(&input, None, NOW);
  assert_eq!(attention_codes(&first), ["threads_actionable"]);
  assert_eq!(first.result["attention"]["new"], true);
  let print = first.result["attention"]["fingerprint"]
    .as_str()
    .unwrap()
    .to_owned();
  assert_eq!(print.len(), 40);
  assert_eq!(first.state["attention"]["fingerprint"], print);
  let again = run(&input, Some(&first.state), LATER);
  assert_eq!(again.result["attention"]["needed"], true);
  assert_eq!(again.result["attention"]["new"], false);
  assert_eq!(again.result["attention"]["fingerprint"], print);
}

#[test]
fn red_ci_a_new_changes_verdict_and_an_escalation_are_attention() {
  let mut red = open_165();
  red["pr"]["statusCheckRollup"][0]["conclusion"] = json!("FAILURE");
  assert_eq!(attention_codes(&run(&red, None, NOW)), ["ci_failed"]);
  let mut changes = open_165();
  changes["bot"]["verdict"]["verdict"] = json!("changes");
  let first = run(&changes, None, NOW);
  assert_eq!(attention_codes(&first), ["review_changes"]);
  let sticky = run(&changes, Some(&first.state), LATER);
  assert!(
    attention_codes(&sticky).is_empty(),
    "a sticky verdict is not new work"
  );
  let merged = run(&fixture::json("snapshots/toolu-165.json"), None, NOW);
  assert_eq!(merged.result["decision"], "escalate");
  assert_eq!(attention_codes(&merged)[0], "escalate");
}

#[test]
fn a_settled_fixer_is_attention_and_a_running_one_is_not() {
  let input = open_165();
  let mut previous = run(&input, None, NOW).state;
  previous["fixer"] = json!({"round": 1, "status": "running", "items": [], "groups": [{}]});
  let running = run(&input, Some(&previous), LATER);
  assert_eq!(attention_codes(&running).len(), 0);
  assert_eq!(running.result["decision"], "keep_going");
  previous["fixer"]["status"] = json!("done");
  let done = run(&input, Some(&previous), LATER);
  assert_eq!(attention_codes(&done), ["fixer_done"]);
}
