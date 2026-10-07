//! `run` over real repositories and checks: freshness, skips, retries, `--step`,
//! `--verify`, scopes, timeouts and the exit-2 failures, as `plan-ledger.js` does them.

use crate::ledger::context::test_repo::Repo;
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;

use super::ledger_run;
use crate::ledger::context::RunFlags;
use crate::ledger::io::{CommandResult, LedgerOptions, read_ledger};

const LEDGER: &str = ".claude/tmp/plan-ledger/feat_x.json";

fn plan(repo: &Repo, steps: &str) {
  let body = format!("# P\n\n## Steps (machine-readable)\n\n```json\n{steps}\n```\n");
  std::fs::write(repo.root.join("plan.md"), body).unwrap();
}

fn flags(step: Option<&str>, verify: bool) -> RunFlags {
  RunFlags {
    only_step: step.map(str::to_owned),
    activity: None,
    force: false,
    verify,
  }
}

fn run(opts: &LedgerOptions, flags: &RunFlags) -> CommandResult {
  ledger_run("plan.md", flags, opts)
}

/// Durations depend on the clock's second boundary; the tests read them as `Ns`.
fn stable(stderr: &str) -> String {
  let mut out = stderr.to_owned();
  for n in 0..5 {
    out = out.replace(&format!("({n}s)"), "(Ns)");
  }
  out
}

fn status_of(repo: &Repo, id: &str) -> String {
  let read = read_ledger(&repo.root.join(LEDGER)).unwrap();
  let Some(Ordered::Array(steps)) = read.value.get("steps") else {
    panic!("no steps in {}", read.text);
  };
  let wanted = Ordered::String(id.to_owned());
  let step = steps.iter().find(|s| s.get("id") == Some(&wanted)).unwrap();
  match step.get("status") {
    Some(Ordered::String(status)) => status.clone(),
    other => panic!("no status: {other:?}"),
  }
}

#[test]
fn a_full_run_stamps_each_step_then_skips_the_fresh_ones() {
  let repo = Repo::new().unwrap();
  plan(
    &repo,
    r#"[{"id":"s1","title":"one","check":"echo ok"},{"id":"s2","title":"two","check":"echo boom >&2; false"}]"#,
  );
  let opts = repo.opts();
  let first = run(&opts, &flags(None, false));
  assert_eq!(
    (first.exit, first.stdout.as_str()),
    (1, "plan-ledger feat_x: 1/2 fresh-green, next=s2\n")
  );
  assert_eq!(
    stable(&first.stderr),
    "plan-ledger: [1/2] s1: running check\nplan-ledger: [1/2] s1: green (Ns)\nplan-ledger: [2/2] s2: running check\nplan-ledger: [2/2] s2: red (Ns)\n"
  );
  let second = run(&opts, &flags(None, false));
  assert!(
    second
      .stderr
      .starts_with("plan-ledger: [1/2] s1: fresh-green, skipped (--force re-runs)\n")
  );
  let written = std::fs::read_to_string(repo.root.join(LEDGER)).unwrap();
  assert!(
    written.contains(
      "\"retries\": [\n        {\n          \"attempt\": 1,\n          \"exit_code\": 1,"
    ),
    "{written}"
  );
  assert!(written.contains("\"evidence_tail\": \"boom\""), "{written}");
  assert!(
    written.ends_with("\"verified_sha\": null\n}\n"),
    "{written}"
  );
  repo.sh("true").unwrap();
}

#[test]
fn a_step_after_a_red_one_runs_and_leaves_the_red_one_next() {
  let repo = Repo::new().unwrap();
  plan(
    &repo,
    r#"[{"id":"s1","title":"1","check":"true"},{"id":"s2","title":"2","check":"false","depends_on":["s1"]},{"id":"s3","title":"3","check":"true","depends_on":["s2"]},{"id":"s4","title":"4","check":"true"}]"#,
  );
  let opts = repo.opts();
  for step in ["s1", "s2"] {
    run(&opts, &flags(Some(step), false));
  }
  let third = run(&opts, &flags(Some("s3"), false));
  assert_eq!(
    (third.exit, third.stdout.as_str()),
    (1, "plan-ledger feat_x: 2/4 fresh-green, next=s2\n")
  );
  assert_eq!(
    stable(&third.stderr),
    "plan-ledger: [1/4] s3: running check\nplan-ledger: [1/4] s3: green (Ns)\n"
  );
  assert_eq!(
    (
      status_of(&repo, "s2"),
      status_of(&repo, "s3"),
      status_of(&repo, "s4")
    ),
    ("red".to_owned(), "green".to_owned(), "pending".to_owned())
  );
}

#[test]
fn the_step_is_running_while_its_check_runs_and_verify_stamps_the_hash() {
  let repo = Repo::new().unwrap();
  let check = format!(r#"grep -q '\"status\": \"running\"' {LEDGER}"#);
  plan(
    &repo,
    &format!(r#"[{{"id":"s1","title":"t","check":"{check}"}}]"#),
  );
  let opts = repo.opts();
  let mut step = flags(Some("s1"), false);
  step.activity = Some("build".to_owned());
  assert_eq!(run(&opts, &step).exit, 0);
  let skipped = run(&opts, &flags(None, true));
  assert!(
    skipped.stderr.contains("s1: fresh-green, skipped"),
    "{}",
    skipped.stderr
  );
  let forced = run(
    &opts,
    &RunFlags {
      force: true,
      ..flags(None, true)
    },
  );
  assert_eq!(
    forced.exit, 1,
    "the check sees no running step now: {}",
    forced.stderr
  );
  plan(&repo, r#"[{"id":"s1","title":"t","check":"true"}]"#);
  assert_eq!(run(&opts, &flags(None, true)).exit, 0);
  let ledger = read_ledger(&repo.root.join(LEDGER)).unwrap();
  assert!(
    matches!(ledger.value.get("verified_sha"), Some(Ordered::String(sha)) if sha.len() == 40)
  );
  assert_eq!(run(&opts, &flags(None, false)).exit, 0);
  let kept = read_ledger(&repo.root.join(LEDGER)).unwrap();
  assert_eq!(
    kept.value.get("verified_sha"),
    ledger.value.get("verified_sha")
  );
}

#[test]
fn a_scoped_step_stays_fresh_until_its_paths_change_or_verify_runs() {
  let repo = Repo::new().unwrap();
  plan(
    &repo,
    r#"[{"id":"s1","title":"t","check":"true","paths":["docs"]}]"#,
  );
  let opts = repo.opts();
  assert_eq!(run(&opts, &flags(None, false)).exit, 0);
  repo.sh("echo y >> a.ts && git commit -qam edit").unwrap();
  let skipped = run(&opts, &flags(None, false));
  assert_eq!(skipped.exit, 0);
  assert!(
    skipped.stderr.contains("fresh-green, skipped"),
    "{}",
    skipped.stderr
  );
  assert!(
    run(&opts, &flags(None, true))
      .stderr
      .contains("s1: running check")
  );
}

#[test]
fn a_check_past_its_bound_is_red_with_the_timeout_reason() {
  let repo = Repo::new().unwrap();
  plan(
    &repo,
    r#"[{"id":"slow","title":"t","check":"echo started; sleep 5"}]"#,
  );
  let opts = LedgerOptions {
    roots: Roots::new(
      repo.env().with("PLAN_LEDGER_STEP_TIMEOUT", "1"),
      Some(Host::Claude),
    ),
    ..repo.opts()
  };
  assert_eq!(run(&opts, &flags(None, false)).exit, 1);
  let written = std::fs::read_to_string(repo.root.join(LEDGER)).unwrap();
  assert!(written.contains("\"exit_code\": 124"), "{written}");
  assert!(
    written.contains("timed out after 1s (PLAN_LEDGER_STEP_TIMEOUT)\\n\\\"started\\\""),
    "{written}"
  );
}

#[test]
fn bad_flags_plans_and_ledgers_fail_with_exit_2() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let activity = RunFlags {
    activity: Some("x".to_owned()),
    ..RunFlags::default()
  };
  let lone = run(&opts, &activity);
  assert_eq!(
    (lone.exit, lone.stderr.as_str()),
    (2, "plan-ledger: --activity requires --step\n")
  );
  let empty = run(&opts, &flags(Some(""), false));
  assert_eq!(empty.stderr, "plan-ledger: --step requires an id\n");
  let missing = run(&opts, &flags(None, false));
  assert_eq!(
    missing.stderr,
    "plan-ledger-parse: plan doc not found: plan.md\n"
  );
  plan(&repo, r#"[{"id":"s1","title":"t","check":"true"}]"#);
  std::fs::create_dir_all(repo.root.join(".claude/tmp/plan-ledger")).unwrap();
  std::fs::write(repo.root.join(LEDGER), "{oops").unwrap();
  let corrupt = run(&opts, &flags(None, false));
  let file = repo.root.join(LEDGER).display().to_string();
  assert_eq!(
    (corrupt.exit, corrupt.stderr),
    (2, format!("plan-ledger: corrupt prior ledger at {file}\n"))
  );
  assert_eq!(corrupt.stdout, "");
}
