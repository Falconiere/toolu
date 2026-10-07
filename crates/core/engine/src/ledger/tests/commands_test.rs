//! `status`, `path`, `root` and `self-test` on real repositories.

use crate::ledger::context::test_repo::Repo;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{
  cutoff, ledger_path_command, ledger_root_command, ledger_self_test, ledger_status, node_join,
  require_git,
};
use crate::ledger::context::RunFlags;
use crate::ledger::io::LedgerOptions;
use crate::ledger::run::ledger_run;

const SPEC: &str =
  "# S\n\n**Status:** Approved\n\n## Acceptance criteria\n- **AC-1:** a\n- **AC-2:** b\n";

#[test]
fn status_heals_recomputes_and_reports_ac_coverage() {
  let repo = Repo::new().unwrap();
  std::fs::write(repo.root.join("spec.md"), SPEC).unwrap();
  let plan = "# P\n\n**Status:** Approved   **Spec:** spec.md\n\n## Steps (machine-readable)\n\n```json\n[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"true\",\"ac_refs\":[\"AC-1\"]},{\"id\":\"s2\",\"title\":\"u\",\"check\":\"true\"}]\n```\n";
  std::fs::write(repo.root.join("plan.md"), plan).unwrap();
  let opts = repo.opts();
  let step = RunFlags {
    only_step: Some("s1".to_owned()),
    ..RunFlags::default()
  };
  assert_eq!(ledger_run("plan.md", &step, &opts).exit, 1);
  let (status, report) = ledger_status(&opts);
  assert_eq!(status.exit, 1);
  assert_eq!(
    status.stdout,
    "plan-ledger feat_x: 1/2 fresh-green, next=s2\nAC coverage (report-only):\n  AC-1: covered by s1\n  AC-2: UNCOVERED (no step references it)\n"
  );
  assert_eq!(report.rows.len(), 2);
  repo.sh("echo y >> a.ts && git commit -qam change").unwrap();
  let (stale, _) = ledger_status(&opts);
  assert!(
    stale
      .stdout
      .starts_with("plan-ledger feat_x: 0/2 fresh-green, next=s1\n"),
    "{}",
    stale.stdout
  );
  assert!(
    stale
      .stdout
      .contains("AC-1: UNCOVERED (s1 not fresh-green)")
  );
}

#[test]
fn status_without_a_ledger_or_repository_fails_with_exit_2() {
  let repo = Repo::new().unwrap();
  let (missing, _) = ledger_status(&repo.opts());
  let file = repo.root.join(".claude/tmp/plan-ledger/feat_x.json");
  assert_eq!(
    (missing.exit, missing.stderr),
    (2, format!("plan-ledger: no ledger at {}\n", file.display()))
  );
  let outside = LedgerOptions {
    cwd: repo.root.join(".home"),
    ..repo.opts()
  };
  repo.sh("rm -rf .git").unwrap();
  let (none, _) = ledger_status(&outside);
  assert_eq!(none.stderr, "plan-ledger: cannot resolve ledger path\n");
  assert_eq!(
    ledger_root_command(&outside).map_err(|r| r.stderr),
    Err("plan-ledger: not in a git repo\n".to_owned())
  );
  assert_eq!(
    ledger_path_command(&outside).map_err(|r| r.stderr),
    Err("plan-ledger: cannot resolve ledger path\n".to_owned())
  );
}

#[test]
fn path_root_self_test_and_git_are_answered() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  assert_eq!(ledger_root_command(&opts).ok(), Some(repo.root.clone()));
  assert_eq!(
    ledger_path_command(&opts).ok(),
    Some(repo.root.join(".claude/tmp/plan-ledger/feat_x.json"))
  );
  let ok = ledger_self_test();
  assert_eq!(
    (ok.exit, ok.stdout.as_str(), ok.stderr.as_str()),
    (0, "plan-ledger --self-test: ok\n", "")
  );
  assert!(require_git(&opts).is_none());
  let no_git = LedgerOptions {
    roots: Roots::new(Env::from_pairs([("PATH", "/nonexistent")]), None),
    ..repo.opts()
  };
  assert_eq!(
    require_git(&no_git).map(|r| (r.exit, r.stderr)),
    Some((2, "plan-ledger: git is required\n".to_owned()))
  );
  assert_eq!(cutoff(&opts), "2031-02-03T04:00:06Z");
  let stuck = |value: &str| {
    cutoff(&LedgerOptions {
      roots: Roots::new(repo.env().with("PL_STUCK_THRESHOLD", value), None),
      ..repo.opts()
    })
  };
  assert_eq!(
    (stuck("60"), stuck("-60"), stuck("1.5")),
    (
      "2031-02-03T04:04:06Z".to_owned(),
      "2031-02-03T04:06:06Z".to_owned(),
      "2031-02-03T04:00:06Z".to_owned()
    )
  );
  assert_eq!(
    node_join(&repo.root, "/docs/x.md"),
    repo.root.join("docs/x.md")
  );
  repo.sh("true").unwrap();
}
