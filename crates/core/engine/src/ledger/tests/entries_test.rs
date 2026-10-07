//! Step entries, against the objects the TypeScript `ledger-model.ts` builds
//! for the same plan step and prior entry.

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use super::{
  RunOutcome, attempt_of, build_step_entry, carry_forward, pending_entry, refresh_authored,
  running_entry,
};
use crate::ledger::jq::parse_json;

fn doc(text: &str) -> Ordered {
  parse_json(text).unwrap()
}

fn text(value: &Ordered) -> String {
  jq_text(value, false)
}

fn step() -> Ordered {
  doc(
    r#"{"id":"s1","title":"T","check":"true","ac_refs":["AC-1"],"depends_on":false,"input":"in","model":"opus","paths":["x"]}"#,
  )
}

const AUTHORED: &str = r#""ac_refs":["AC-1"],"depends_on":[],"input":"in","model":"opus""#;

#[test]
fn a_pending_seed_and_a_running_pre_write() {
  assert_eq!(
    text(&pending_entry(&step()).unwrap()),
    format!(
      r#"{{"id":"s1","title":"T","check":"true","status":"pending","started_at":null,"activity":null,"exit_code":null,"diff_sha":null,"last_run":null,"evidence_tail":null,{AUTHORED},"retries":[]}}"#
    )
  );
  let prior = doc(r#"{"retries":[{"attempt":1}]}"#);
  assert_eq!(
    text(&running_entry(&step(), &prior, "NOW", "build").unwrap()),
    format!(
      r#"{{"id":"s1","title":"T","check":"true","status":"running","started_at":"NOW","activity":"build","exit_code":null,"diff_sha":null,"last_run":"NOW","evidence_tail":null,{AUTHORED},"retries":[{{"attempt":1}}]}}"#
    )
  );
  let fresh = running_entry(&step(), &Ordered::Null, "NOW", "").unwrap();
  assert_eq!(fresh.get("activity"), Some(&Ordered::Null));
  assert_eq!(text(fresh.get("retries").unwrap()), "[]");
}

#[test]
fn refresh_and_carry_forward_reapply_the_authored_fields() {
  let prior = doc(r#"{"id":"s1","status":"green","extra":1,"model":"haiku"}"#);
  assert_eq!(
    text(&refresh_authored(&prior, &step()).unwrap()),
    r#"{"id":"s1","status":"green","extra":1,"model":"opus","ac_refs":["AC-1"],"depends_on":[],"input":"in"}"#
  );
  let stale = doc(r#"{"id":"s1","status":false,"extra":1,"retries":null,"title":"old"}"#);
  assert_eq!(
    text(&carry_forward(&stale, &step()).unwrap()),
    r#"{"id":"s1","status":"pending","extra":1,"retries":[],"title":"T","scope_sha":null,"ac_refs":["AC-1"],"depends_on":[],"input":"in","model":"opus","check":"true","started_at":null,"activity":null,"exit_code":null,"diff_sha":null,"last_run":null,"evidence_tail":null}"#
  );
  assert_eq!(
    text(&carry_forward(&Ordered::Null, &step()).unwrap()),
    format!(
      r#"{{"scope_sha":null,{AUTHORED},"title":"T","check":"true","status":"pending","started_at":null,"activity":null,"exit_code":null,"diff_sha":null,"last_run":null,"evidence_tail":null,"retries":[]}}"#
    )
  );
  assert!(refresh_authored(&doc("[1]"), &step()).is_err());
  assert!(carry_forward(&doc("\"x\""), &step()).is_err());
}

fn outcome(status: &'static str, exit_code: i32, evidence: &str) -> RunOutcome {
  RunOutcome {
    status,
    exit_code,
    sha: "C".to_owned(),
    evidence: evidence.to_owned(),
    now: "NOW".to_owned(),
  }
}

#[test]
fn a_finished_step_archives_a_prior_red_attempt() {
  let red = doc(
    r#"{"status":"red","exit_code":1,"diff_sha":"P","evidence_tail":"boom","last_run":"THEN","retries":[{"attempt":1}]}"#,
  );
  let built = build_step_entry(&step(), &red, &outcome("green", 0, "ok")).unwrap();
  assert_eq!(
    text(&built),
    format!(
      r#"{{"id":"s1","title":"T","check":"true","status":"green","started_at":null,"activity":null,"exit_code":0,"diff_sha":"C","last_run":"NOW","evidence_tail":"ok",{AUTHORED},"retries":[{{"attempt":1}},{{"attempt":2,"exit_code":1,"diff_sha":"P","evidence_tail":"boom","at":"THEN"}}]}}"#
    )
  );
  assert_eq!(attempt_of(&built), 3);
  let green = doc(r#"{"status":"green","retries":"zz"}"#);
  let kept = build_step_entry(&step(), &green, &outcome("red", 2, "")).unwrap();
  assert_eq!(text(kept.get("retries").unwrap()), r#""zz""#);
  assert_eq!(attempt_of(&kept), 1);
  let bad = doc(r#"{"status":"red","retries":5}"#);
  assert!(build_step_entry(&step(), &bad, &outcome("red", 2, "")).is_err());
}
