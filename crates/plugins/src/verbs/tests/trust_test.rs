use std::time::Duration;

use super::settle;
use crate::args::Approval;
use crate::host::adapter::Adapter;
use crate::step::{Status, Step};
use crate::support::Sandbox;
use crate::verbs::Driver;

fn step(name: &str, status: Status) -> Step {
  Step::new(name, status, "installed", Vec::new())
}

fn details(steps: &[Step]) -> Vec<(Status, &str)> {
  steps
    .iter()
    .map(|step| (step.status, step.detail.as_str()))
    .collect()
}

fn muse(sb: &Sandbox, approval: Approval) -> Driver {
  Driver {
    adapter: Adapter::Muse,
    env: sb.env(),
    timeout: Duration::from_secs(10),
    checkout: None,
    approval,
  }
}

#[test]
fn without_approve_a_pending_plugin_names_the_command_and_succeeds() {
  let sb = Sandbox::new();
  sb.muse("never");
  sb.capture("inspect-review-needed", "muse-inspect-hooky.json");
  sb.capture("inspect-trusted", "muse-inspect-toolu.json");
  let steps = settle(
    &muse(&sb, Approval::Report),
    vec![
      step("hooky", Status::Installed),
      step("toolu", Status::Already),
    ],
  );
  assert_eq!(
    details(&steps),
    [
      (
        Status::Installed,
        "installed; awaiting approval: muse plugins approve hooky"
      ),
      (Status::Already, "installed"),
    ]
  );
  assert!(!sb.log("muse").iter().any(|line| line.contains("approve")));
}

#[test]
fn approve_trusts_review_needed_and_modified_plugins_only() {
  let sb = Sandbox::new();
  sb.muse("never");
  sb.capture("inspect-review-needed", "muse-inspect-hooky.json");
  sb.capture("inspect-modified", "muse-inspect-jev.json");
  sb.capture("inspect-toolu-trusted", "muse-inspect-toolu.json");
  let steps = settle(
    &muse(&sb, Approval::Approve),
    vec![
      step("hooky", Status::Installed),
      step("jev", Status::Updated),
      step("toolu", Status::Current),
    ],
  );
  assert_eq!(
    details(&steps),
    [
      (Status::Installed, "installed; approved"),
      (Status::Updated, "installed; approved"),
      (Status::Current, "installed"),
    ]
  );
  let approvals: Vec<String> = sb
    .log("muse")
    .into_iter()
    .filter(|line| line.contains("approve"))
    .collect();
  assert_eq!(
    approvals,
    ["plugins approve hooky --json", "plugins approve jev --json"]
  );
}

#[test]
fn an_unknown_trust_state_or_a_failed_approval_fails_the_step() {
  let sb = Sandbox::new();
  sb.muse("\"plugins inspect gone --json\"|\"plugins approve hooky --json\"");
  sb.capture("inspect-review-needed", "muse-inspect-hooky.json");
  sb.write("muse-inspect-odd.json", r#"{"plugins":[]}"#);
  let steps = settle(
    &muse(&sb, Approval::Approve),
    vec![
      step("gone", Status::Installed),
      step("hooky", Status::Installed),
      step("odd", Status::Already),
    ],
  );
  assert_eq!(
    details(&steps),
    [
      (
        Status::Failed,
        "installed, but `muse plugins inspect gone --json` failed: boom: plugins inspect gone --json"
      ),
      (
        Status::Failed,
        "installed, but `muse plugins approve hooky --json` failed: boom: plugins approve hooky --json"
      ),
      (
        Status::Failed,
        "installed, but `muse plugins inspect odd --json` failed: expected an object with a \"runtime_capabilities\" array"
      ),
    ]
  );
}

#[test]
fn absent_plugins_and_hosts_without_approval_are_untouched() {
  let sb = Sandbox::new();
  sb.muse("never");
  sb.host("claude", "[]", "never");
  let untouched = vec![
    step("a", Status::Failed),
    step("b", Status::Skipped),
    step("c", Status::Skew),
  ];
  assert_eq!(
    settle(&muse(&sb, Approval::Approve), untouched.clone()),
    untouched
  );
  assert_eq!(sb.log("muse"), Vec::<String>::new());
  let claude = Driver {
    adapter: Adapter::Claude,
    ..muse(&sb, Approval::Approve)
  };
  let installed = vec![step("toolu", Status::Installed)];
  assert_eq!(settle(&claude, installed.clone()), installed);
  assert_eq!(sb.log("claude"), Vec::<String>::new());
}
