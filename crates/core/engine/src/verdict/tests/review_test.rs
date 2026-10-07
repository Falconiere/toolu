//! The verdict's review gate over real push-review state files.

use toolu_runtime::json::ordered::Ordered;

use super::review_gate;
use crate::ledger::context::test_repo::Repo;
use crate::verdict::gates::GateContext;

const STATE: &str = ".claude/tmp/push-review/feat_x.json";

fn ctx<'a>(
  repo: &Repo,
  roots: &'a toolu_runtime::host::roots::Roots,
  cur: &str,
) -> GateContext<'a> {
  GateContext {
    roots,
    root: repo.root.clone(),
    branch: "feat/x".to_owned(),
    base: "main".to_owned(),
    cur: cur.to_owned(),
    cwd: repo.root.clone(),
    warnings: Vec::new(),
  }
}

fn verdict(gate: &Ordered) -> String {
  let text = |key| match gate.get(key) {
    Some(Ordered::String(text)) => text.clone(),
    Some(other) => other.to_text(false),
    None => String::new(),
  };
  format!(
    "{} {} [{} {}]",
    text("state"),
    text("reason"),
    text("reason_code"),
    text("round")
  )
}

fn state(repo: &Repo, body: &str) {
  repo.sh("mkdir -p .claude/tmp/push-review").unwrap();
  std::fs::write(repo.root.join(STATE), body).unwrap();
}

fn v2(extra: &str) -> String {
  format!(
    r#"{{"version":2,"diff_sha":"C","findings_count":0,"reviewers":["code-review"],"reviewed_files":["a.ts"]{extra}}}"#
  )
}

#[test]
fn the_review_gate_skips_or_fails_before_reading_a_state() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let mut on_base = ctx(&repo, &opts.roots, "C");
  on_base.branch = "main".to_owned();
  assert_eq!(
    verdict(&review_gate(&on_base)),
    "skip current branch is the base branch [null null]"
  );
  assert_eq!(
    verdict(&review_gate(&ctx(&repo, &opts.roots, ""))),
    "skip could not compute diff against main [null null]"
  );
  let empty = review_gate(&ctx(
    &repo,
    &opts.roots,
    "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
  ));
  assert_eq!(
    verdict(&empty),
    "fail diff against main is empty; verify intent before pushing [empty-diff null]"
  );
  let missing = review_gate(&ctx(&repo, &opts.roots, "C"));
  assert_eq!(
    verdict(&missing),
    "fail no push-review state file; run a reviewer and write the state [no-state null]"
  );
  state(&repo, "false");
  let file = repo.root.join(STATE).display().to_string();
  assert_eq!(
    verdict(&review_gate(&ctx(&repo, &opts.roots, "C"))),
    format!("fail push-review state file is unparseable at {file} [schema null]")
  );
}

#[test]
fn a_v2_state_passes_only_when_current_complete_and_clean() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let file = repo.root.join(STATE).display().to_string();
  let cases = [
    (r#"{"version":1}"#.to_owned(), "fail push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file [schema-v1 1]".to_owned()),
    (r#"{"version":2,"diff_sha":"C"}"#.to_owned(), format!("fail push-review state file is corrupted or missing required v2 fields at {file} [schema 1]")),
    (v2(r#","reviewers":["me"]"#), "fail state file lists no accepted reviewer [reviewer 1]".to_owned()),
    (v2(r#","review_round":6.5"#), "escalate review loop hit 6 rounds (max 5) on an unchanged diff [round-cap 6.5]".to_owned()),
    (v2(r#","review_round":5,"diff_sha":"OLD""#), "fail diff changed since review (state stale) [stale-diff 5]".to_owned()),
    (v2(r#","reviewed_files":["b.ts"]"#), "fail reviewed_files does not match the current diff's changed paths [file-coverage 1]".to_owned()),
    (v2(r#","findings_count":2"#), "fail code review has open findings (2) [findings 1]".to_owned()),
    (v2(r#","review_round":"x""#), "pass review satisfied [pass null]".to_owned()),
    (v2(""), "pass review satisfied [pass 1]".to_owned()),
  ];
  for (body, expected) in cases {
    state(&repo, &body);
    assert_eq!(
      verdict(&review_gate(&ctx(&repo, &opts.roots, "C"))),
      expected,
      "{body}"
    );
  }
}
