use std::time::Duration;

use super::run;
use crate::args::Approval;
use crate::host::adapter::Adapter;
use crate::step::Status;
use crate::support::{Sandbox, names};
use crate::verbs::Driver;

#[test]
fn each_name_is_removed_independently_and_a_dry_run_runs_nothing() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "\"plugin uninstall jev@toolu\"");
  let driver = Driver {
    adapter: Adapter::Claude,
    env: sb.env(),
    timeout: Duration::from_secs(10),
    checkout: None,
    pin: None,
    approval: Approval::Report,
  };
  let steps = run(&driver, &names(&["ast-grep", "jev"]), false);
  let seen: Vec<(Status, &str)> = steps
    .iter()
    .map(|step| (step.status, step.detail.as_str()))
    .collect();
  assert_eq!(
    seen,
    [
      (Status::Removed, "removed"),
      (Status::Failed, "boom: plugin uninstall jev@toolu")
    ]
  );
  let before = sb.log("claude").len();
  let dry = run(&driver, &names(&["ast-grep"]), true);
  assert_eq!(
    (dry[0].status, dry[0].detail.as_str()),
    (Status::Skipped, "dry run")
  );
  assert_eq!(sb.log("claude").len(), before);
}
