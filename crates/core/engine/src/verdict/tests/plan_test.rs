//! The verdict's plan gate over real ledgers and repositories.

use toolu_runtime::json::ordered::Ordered;

use super::plan_gate;
use crate::ledger::context::test_repo::Repo;

const LEDGER: &str = ".claude/tmp/plan-ledger/feat_x.json";

fn ledger(repo: &Repo, body: &str) {
  repo.sh("mkdir -p .claude/tmp/plan-ledger").unwrap();
  std::fs::write(repo.root.join(LEDGER), body).unwrap();
}

#[test]
fn without_a_ledger_code_changes_advise_and_others_skip() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let advise = plan_gate(&repo.gate(&opts.roots, "feat/x", "C")).to_text(false);
  assert_eq!(
    advise,
    r#"{"state":"advise","reason":"no plan ledger for this change; if non-trivial, run plan","summary":{"total":0,"fresh_green":0},"ac_uncovered":0}"#
  );
  repo
    .sh("git rm -q a.ts && echo d > notes.md && git add notes.md && git commit -qm docs")
    .unwrap();
  let skip = plan_gate(&repo.gate(&opts.roots, "feat/x", "C"));
  assert_eq!(
    skip.get("reason"),
    Some(&Ordered::String(
      "no plan ledger and no code files in diff".to_owned()
    ))
  );
}

#[test]
fn broken_and_empty_ledgers_are_judged() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let reason = |body: &str| {
    ledger(&repo, body);
    let gate = plan_gate(&repo.gate(&opts.roots, "feat/x", "C"));
    match (gate.get("state"), gate.get("reason")) {
      (Some(Ordered::String(state)), Some(Ordered::String(reason))) => format!("{state}: {reason}"),
      _ => String::new(),
    }
  };
  let file = repo.root.join(LEDGER).display().to_string();
  assert_eq!(
    reason("{nope"),
    format!("fail: unparseable ledger at {file}")
  );
  assert_eq!(
    reason(r#"{"version":2}"#),
    format!("fail: ledger schema mismatch at {file} (version=\"2\", expected 1)")
  );
  assert_eq!(
    reason(r#"{"version":1,"summary":{"total":0},"steps":[1]}"#),
    "skip: empty plan ledger"
  );
  assert_eq!(
    reason(r#"{"version":1,"summary":{"total":1},"steps":[]}"#),
    "skip: empty plan ledger"
  );
}

#[test]
fn steps_are_fresh_stale_or_blocking_and_acs_are_counted() {
  let repo = Repo::new().unwrap();
  std::fs::write(
    repo.root.join("spec.md"),
    "## Acceptance criteria\n- **AC-1:** a\n- **AC-2:** b\n",
  )
  .unwrap();
  std::fs::write(repo.root.join("plan.md"), "**Spec:** spec.md\n").unwrap();
  let opts = repo.opts();
  ledger(
    &repo,
    r#"{"version":1,"plan_doc":"plan.md","summary":{"total":3},"steps":[
{"id":"s1","status":"green","diff_sha":"C","ac_refs":["AC-1"]},
{"id":"s2","status":"green","diff_sha":"OLD","ac_refs":["AC-2"]},
{"id":"s3","status":"red"},{"status":null}]}"#,
  );
  let gate = plan_gate(&repo.gate(&opts.roots, "feat/x", "C")).to_text(false);
  assert_eq!(
    gate,
    r#"{"state":"fail","reason":"steps not fresh-green: s2: stale,s3: red,?: pending","summary":{"total":4,"fresh_green":1},"ac_uncovered":1}"#
  );
  ledger(
    &repo,
    r#"{"version":1,"summary":{"total":1},"steps":[{"id":"s1","status":"green","diff_sha":"C"}]}"#,
  );
  let pass = plan_gate(&repo.gate(&opts.roots, "feat/x", "C")).to_text(false);
  assert_eq!(
    pass,
    r#"{"state":"pass","reason":"all plan-ledger steps fresh-green","summary":{"total":1,"fresh_green":1},"ac_uncovered":0}"#
  );
  ledger(&repo, r#"{"version":1,"summary":{"total":1},"steps":[3]}"#);
  let jq_error = plan_gate(&repo.gate(&opts.roots, "feat/x", "C")).to_text(false);
  assert_eq!(
    jq_error,
    r#"{"state":"pass","reason":"all plan-ledger steps fresh-green"}"#
  );
}
