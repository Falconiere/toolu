use std::time::Duration;

use super::run;
use crate::host::adapter::Adapter;
use crate::step::Status;
use crate::support::{Sandbox, names};
use crate::verbs::Driver;

fn driver(sb: &Sandbox) -> Driver {
  Driver {
    adapter: Adapter::Claude,
    env: sb.env(),
    timeout: Duration::from_secs(10),
  }
}

#[test]
fn only_plugins_behind_the_offer_are_updated_in_the_hosts_order() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "\"plugin update ast-grep@toolu\"");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.10.0"},{"id":"toolu@toolu","version":"7.11.0"},{"id":"ast-grep@toolu","version":"1"}]"#,
  );
  sb.write(
    "claude-available.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"},{"id":"toolu@toolu","version":"7.11.0"}]"#,
  );
  let Ok(steps) = run(&driver(&sb), &[], false) else {
    panic!("update failed outright");
  };
  let seen: Vec<(&str, Status, &str)> = steps
    .iter()
    .map(|step| (step.name.as_str(), step.status, step.detail.as_str()))
    .collect();
  assert_eq!(
    seen,
    [
      ("jev", Status::Updated, "updated to 7.11.0"),
      ("toolu", Status::Current, "current at 7.11.0"),
      (
        "ast-grep",
        Status::Failed,
        "boom: plugin update ast-grep@toolu"
      ),
    ]
  );
}

#[test]
fn a_named_plugin_the_host_does_not_report_is_still_attempted() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let Ok(steps) = run(&driver(&sb), &names(&["jev"]), false) else {
    panic!("update failed outright");
  };
  assert_eq!(
    (steps[0].status, steps[0].detail.as_str()),
    (Status::Updated, "updated")
  );
  let Ok(dry) = run(&driver(&sb), &names(&["jev"]), true) else {
    panic!("the dry run failed");
  };
  assert_eq!(dry[0].status, Status::Skipped);
}

#[test]
fn nothing_requested_and_nothing_installed_says_so() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let Err(fail) = run(&driver(&sb), &[], false) else {
    panic!("an empty update reported success");
  };
  assert_eq!(
    fail.message,
    "claude reported no installed plugins, so there is nothing to update"
  );
}
