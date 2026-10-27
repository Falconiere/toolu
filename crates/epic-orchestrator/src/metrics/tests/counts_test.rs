use serde_json::json;

use super::Counts;
use crate::journal::Record;

fn counted(rows: &[(&str, &str, &str, &str, &str, u64)]) -> serde_json::Value {
  let mut counts = Counts::default();
  for (issue, kind, name, token, note, at) in rows {
    counts.push(issue, &Record::new(kind, name, issue, token, note), *at);
  }
  counts.finish(2)
}

#[test]
fn prompts_need_a_cause_or_they_count_against_the_budgets() {
  let doc = counted(&[
    (
      "a",
      "transition",
      "github-check",
      "",
      r#"{"next":"prompt"}"#,
      1,
    ),
    (
      "a",
      "transition",
      "github-check",
      "",
      r#"{"next":"keep-going"}"#,
      2,
    ),
    ("a", "prompt", "babysit", "", "fix the test", 3),
    ("a", "prompt", "babysit", "", "keep going", 4),
    ("a", "prompt", "status", "", "STATUS?", 5),
    ("b", "action", "merge-done", "", "handoff: conflict", 6),
    ("b", "prompt", "rebase", "", "REBASE", 7),
    ("b", "prompt", "rebase", "", "REBASE", 8),
    ("c", "prompt", "answer", "", "yes", 9),
  ]);
  assert_eq!(
    doc["prompts"],
    json!({
      "by_reason": {"answer": 1, "babysit": 2, "rebase": 2, "status": 1},
      "decided_checks": 2, "while_waiting": 2, "rebase_without_conflict": 1,
    })
  );
}

#[test]
fn judgments_count_distinct_tokens_and_closed_waits() {
  let doc = counted(&[
    ("a", "judgment", "needs-human", "t1", "first", 1),
    ("a", "judgment", "needs-human", "t1", "changed note", 2),
    ("b", "judgment", "needs-human", "t2", "", 3),
    (
      "a",
      "judgment",
      "closed",
      "t1",
      "kind=needs-human waited_ms=5000 by=answer",
      4,
    ),
    ("a", "judgment", "reminder", "t1", "", 5),
    // An answer with no open item raises nothing.
    ("a", "judgment", "directed", "", "by=web", 5),
    ("x", "action", "wait-reply", "", "judgment:needs-human", 6),
    ("x", "action", "wait-reply", "", "timeout", 7),
  ]);
  assert_eq!(
    doc["judgments"],
    json!({"needs-human": {"raised": 2, "closed": 1,
      "wait_ms": {"count": 1, "p50": 5000, "p90": 5000, "max": 5000}}})
  );
  assert_eq!(doc["wakeups"], json!({"total": 2, "judgment_free": 1}));
}

#[test]
fn job_waits_run_from_the_first_refusal_to_the_admitted_attempt() {
  let doc = counted(&[
    ("a", "admission", "job", "", "cargo test", 100),
    ("a", "admission", "job-wait", "", "capacity", 101),
    ("a", "admission", "job", "", "cargo test", 160),
    ("a", "admission", "job-wait", "", "pressure", 161),
    ("a", "admission", "job", "", "cargo test", 400),
    ("b", "admission", "job", "", "cargo test", 500),
    ("x", "action", "reroute", "", "", 1),
    ("x", "action", "host-limited", "", "", 1),
    ("x", "action", "launch-wait", "", "no host capacity", 1),
    ("x", "action", "launch-wait", "", "3", 2),
  ]);
  assert_eq!(
    doc["jobs"],
    json!({"attempts": 4, "waits": 2, "pressure_holds": 1,
      "wait_ms": {"count": 1, "p50": 300, "p90": 300, "max": 300}})
  );
  assert_eq!(
    doc["launches"],
    json!({"relaunches": 2, "host_switches": 1, "cooldowns": 1, "waits": 1})
  );
}

#[test]
fn engine_samples_sum_cpu_over_lifetimes_and_rebases_split_by_performer() {
  let mut counts = Counts::default();
  let rows = [
    (
      "action",
      "engine-usage",
      r#"{"event":"start","max_rss_bytes":100,"cpu_ms":1}"#,
    ),
    (
      "action",
      "engine-usage",
      r#"{"event":"sample","max_rss_bytes":300,"cpu_ms":40}"#,
    ),
    (
      "action",
      "engine-usage",
      r#"{"event":"start","max_rss_bytes":200,"cpu_ms":2}"#,
    ),
    (
      "action",
      "engine-usage",
      r#"{"event":"stop","max_rss_bytes":250,"cpu_ms":10}"#,
    ),
    ("action", "rebase", r#"{"by":"engine","round":1}"#),
    ("action", "rebase", r#"{"by":"worker","round":1}"#),
    ("action", "rebase", "not json"),
  ];
  for (kind, name, note) in rows {
    counts.push("i", &Record::new(kind, name, "i", "", note), 1);
  }
  assert_eq!(
    counts.rounds(),
    json!({"engine": 1, "worker": 1, "issues": {"i": {"engine": 1, "worker": 1}}})
  );
  let doc = counts.finish(0);
  assert_eq!(
    doc["engine"],
    json!({"lifetimes": 2, "max_rss_bytes": 300, "cpu_ms": 50})
  );
}
