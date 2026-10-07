//! One ledger across implementations (#421, AC-2): a ledger the TypeScript
//! `plan-ledger` bundle stamped is continued by `toolu ledger`, verified by
//! the bundle again, and the reverse. The two orders write the same bytes,
//! timestamps aside, and keep each other's retries, `scope_sha` and
//! `verified_sha`.

#[path = "helpers/interop.rs"]
mod interop;
#[path = "helpers/ledger.rs"]
mod ledger;

use interop::{at, plan, rust, typescript};
use ledger::{Project, Res};
use regex::Regex;
use serde_json::json;

/// A paths-scoped step with a model, and a step gated on a file git ignores.
fn steps() -> serde_json::Value {
  json!([
    {"id": "s1", "title": "scoped", "check": "true", "paths": ["a.ts"], "ac_refs": ["AC-1"], "model": "sonnet"},
    {"id": "s2", "title": "gated", "check": "test -f .git/gate.ok", "depends_on": ["s1"], "ac_refs": ["AC-2"]},
  ])
}

/// What one implementation started and the other continued.
struct Continuation {
  /// The first run, the second, and the first's `--verify`: exit codes, and the second's stdout.
  exits: (Option<i32>, Option<i32>, Option<i32>, String),
  /// s1's `last_run` after the first run and after the second.
  s1_runs: (String, String),
  /// `status` from each implementation over the final ledger.
  statuses: [(Option<i32>, String); 2],
  /// The final ledger with its timestamps as `<TS>`.
  ledger: String,
}

/// `first` stamps s1 green and s2 red, `second` continues once the gate
/// opens, then `first` verifies the branch.
fn continued(first: &str, second: &str) -> Res<Continuation> {
  let project = Project::new()?;
  plan(&project, &steps())?;
  let run = |who: &str, args: &[&str]| -> Res<(Option<i32>, String)> {
    let output = if who == "ts" {
      typescript(&project, args)?
    } else {
      rust(&project, args)?
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok((output.status.code(), stdout))
  };
  let last_run = || -> Res<String> { Ok(at(&project, "/steps/0/last_run")?.to_string()) };
  let (red, _) = run(first, &["run", "plan.md"])?;
  let before = last_run()?;
  project.sh("touch .git/gate.ok")?;
  let (green, stdout) = run(second, &["run", "plan.md"])?;
  let after = last_run()?;
  let (verified, _) = run(first, &["run", "plan.md", "--verify"])?;
  let timestamp = Regex::new(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ")?;
  let ledger = std::fs::read_to_string(project.ledger())?;
  Ok(Continuation {
    exits: (red, green, verified, stdout),
    s1_runs: (before, after),
    statuses: [run("ts", &["status"])?, run("rust", &["status"])?],
    ledger: timestamp.replace_all(&ledger, "<TS>").into_owned(),
  })
}

#[test]
fn a_typescript_ledger_continues_in_rust_and_the_reverse_with_the_same_bytes() {
  let mut ledgers = Vec::new();
  for (first, second) in [("ts", "rust"), ("rust", "ts")] {
    let done = continued(first, second).unwrap();
    let all_green = "plan-ledger feat_x: 2/2 fresh-green, next=none\n".to_owned();
    let order = format!("{first} then {second}");
    assert_eq!(
      done.exits,
      (Some(1), Some(0), Some(0), all_green),
      "{order}"
    );
    assert_eq!(done.s1_runs.0, done.s1_runs.1, "{order}: s1 was rerun");
    assert_eq!(
      done.statuses[0], done.statuses[1],
      "{order}: status differs"
    );
    let ledger: serde_json::Value = serde_json::from_str(&done.ledger).unwrap();
    assert!(ledger["verified_sha"].is_string(), "{order}: {ledger:#}");
    assert!(ledger["steps"][0]["scope_sha"].is_string(), "{order}");
    assert_eq!(ledger["steps"][0]["model"], "sonnet");
    assert_eq!(ledger["steps"][1]["retries"][0]["exit_code"], 1, "{order}");
    ledgers.push(done.ledger);
  }
  assert_eq!(
    ledgers[0], ledgers[1],
    "the two orders wrote different ledgers"
  );
}
