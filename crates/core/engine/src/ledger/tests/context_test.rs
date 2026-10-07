//! The run context: flags, the resolved repository and the prior ledger.

use crate::ledger::context::test_repo::Repo;

use super::{CommandFail, RunFlags, base_for, or_fail, prepare};
use crate::ledger::jq::JqError;

fn fail(line: &str) -> CommandFail {
  CommandFail::line(line)
}

#[test]
fn flags_follow_typescripts_rules() {
  let with = |step: Option<&str>, activity: Option<&str>| RunFlags {
    only_step: step.map(str::to_owned),
    activity: activity.map(str::to_owned),
    ..RunFlags::default()
  };
  assert_eq!(with(Some("s1"), Some("build")).check(), Ok(()));
  assert_eq!(
    with(Some(""), None).check(),
    Err(fail("plan-ledger: --step requires an id"))
  );
  assert_eq!(
    with(Some("s1"), Some("")).check(),
    Err(fail("plan-ledger: --activity requires a label"))
  );
  assert_eq!(
    with(None, Some("x")).check(),
    Err(fail("plan-ledger: --activity requires --step"))
  );
  assert_eq!(
    (with(None, None).step(), with(None, None).activity()),
    ("", "")
  );
  let failed: Result<(), JqError> = Err(JqError("x".to_owned()));
  assert_eq!(or_fail(failed, "tagged"), Err(fail("tagged")));
}

#[test]
fn the_context_resolves_the_repository_hash_and_prior_entries() {
  let repo = Repo::new().unwrap();
  std::fs::write(
    repo.root.join("plan.md"),
    "## Steps (machine-readable)\n```json\n[{\"id\":\"s1\",\"title\":\"t\",\"check\":\"true\"}]\n```\n",
  ).unwrap();
  let opts = repo.opts();
  let flags = RunFlags::default();
  let ctx = prepare("plan.md", &flags, &opts).unwrap();
  assert_eq!(
    (ctx.base.as_str(), ctx.branch.as_str(), ctx.timeout.as_str()),
    ("main", "feat/x", "1800")
  );
  assert_eq!(
    ctx.ledger_file,
    repo.root.join(".claude/tmp/plan-ledger/feat_x.json")
  );
  assert_eq!(ctx.cur.len(), 40);
  assert_eq!(ctx.now(), "2031-02-03T04:05:06Z");
  assert!(ctx.existing.is_empty() && ctx.prior.is_none());
  repo.sh("mkdir -p .claude/tmp/plan-ledger && echo '{\"steps\":[{\"id\":7}]}' > .claude/tmp/plan-ledger/feat_x.json").unwrap();
  let corrupt = prepare("plan.md", &flags, &opts).map(|_| ());
  let file = ctx.ledger_file.display().to_string();
  assert_eq!(
    corrupt,
    Err(fail(&format!(
      "plan-ledger: corrupt prior ledger at {file}"
    )))
  );
  assert_eq!(base_for(&opts, None), "main");
}
