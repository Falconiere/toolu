//! The skills that size, plan, execute and babysit a pull request hold it to the review
//! budget (`gates.prSize`). Each test pins one rule; wrapping and indentation do not count.

use std::path::PathBuf;

type Res = Result<(), String>;

const ISSUES: &str = "plugins/epic-creator/skills/epic-creator/references/issue-bodies.md";
const REFS: &str = "plugins/delivery-flow/skills/delivery-flow/references";

fn read(rel: &str) -> Result<String, String> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../..")
    .join(rel);
  std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))
}

/// `rule` appears in `rel` once runs of whitespace are folded to one space.
fn pinned(rel: &str, rule: &str) -> Res {
  let flat = read(rel)?.split_whitespace().collect::<Vec<_>>().join(" ");
  let found = flat.contains(rule);
  found
    .then_some(())
    .ok_or_else(|| format!("{rel} lost the rule: {rule}"))
}

#[test]
fn every_sub_issue_template_states_its_size_estimate() -> Res {
  let line = "Base: <sha>\nSize: ~<n> code lines\n";
  let found = read(ISSUES)?.contains(line);
  found
    .then_some(())
    .ok_or_else(|| format!("{ISSUES} template lacks {line:?}"))
}

#[test]
fn a_sub_issue_over_the_budget_is_split_before_it_is_filed() -> Res {
  pinned(
    ISSUES,
    "the pr-size gate denies the worker's push, so split first",
  )
}

#[test]
fn a_plan_states_its_size_estimate() -> Res {
  let rule = "add a `**Size:** ~<n> of <budget> code lines` line below the header";
  pinned(&format!("{REFS}/plan.md"), rule)
}

#[test]
fn an_over_budget_plan_keeps_one_slice_and_lists_the_rest_as_follow_ups() -> Res {
  let rule = "Over the budget, slice before planning further. The plan covers only the first slice";
  pinned(&format!("{REFS}/plan.md"), rule)?;
  pinned(
    &format!("{REFS}/plan.md"),
    "List every later slice in one line under `## Follow-ups`",
  )
}

#[test]
fn plan_review_blocks_an_over_budget_plan_without_follow_ups() -> Res {
  let rule =
    "An over-budget plan that is not cut to a first slice with `## Follow-ups` is a blocker";
  pinned(&format!("{REFS}/plan-review.md"), rule)
}

#[test]
fn execution_reports_size_advice_and_honors_a_failing_size_gate() -> Res {
  let rule = "Report advisory findings. When the row fails, keep a working slice";
  pinned(&format!("{REFS}/execution.md"), rule)?;
  let audit = "advance when it reports `overall: ready`. Report any size advisory.";
  pinned(&format!("{REFS}/execution.md"), audit)
}

#[test]
fn babysit_declines_review_requests_beyond_the_goal_as_follow_ups() -> Res {
  let babysit = "plugins/pr-babysit/workflows/babysit.md";
  pinned(babysit, "A request for surface the goal does not need")?;
  let decline =
    "is Won't fix as YAGNI, with the reason in the reply and the item in the report as a follow-up";
  pinned(babysit, decline)
}
