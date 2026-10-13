use super::{Status, Step};

#[test]
fn only_a_failed_step_fails_and_a_skew_does_not() {
  let statuses = [
    (Status::Installed, "installed", '+'),
    (Status::Already, "already", '='),
    (Status::Skew, "skew", '!'),
    (Status::Failed, "failed", 'x'),
    (Status::Skipped, "skipped", '-'),
    (Status::Updated, "updated", '+'),
    (Status::Current, "current", '='),
    (Status::Removed, "removed", '-'),
  ];
  for (status, name, mark) in statuses {
    let step = Step::new("p", status, "d", Vec::new());
    assert_eq!((status.name(), status.mark(), step.failed()), (name, mark, status == Status::Failed));
  }
  let planned = Step::new("jev", Status::Installed, "enabled", Vec::new()).planned("enable jev in /s");
  assert_eq!(planned.plan.as_deref(), Some("enable jev in /s"));
}
