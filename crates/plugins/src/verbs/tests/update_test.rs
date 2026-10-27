use std::time::Duration;

use super::run;
use crate::args::Approval;
use crate::host::adapter::Adapter;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};
use crate::verbs::Driver;

fn driver(sb: &Sandbox) -> Driver {
  Driver {
    adapter: Adapter::Claude,
    env: sb.env(),
    timeout: Duration::from_secs(10),
    checkout: None,
    pin: None,
    approval: Approval::Report,
  }
}

/// The `plugin update` lines the claude stub logged.
fn updates(sb: &Sandbox) -> Vec<String> {
  sb.log("claude")
    .into_iter()
    .filter(|line| line.starts_with("plugin update"))
    .collect()
}

#[test]
fn only_toolu_plugins_are_updated_once_each_and_retired_ones_get_a_hint() {
  let sb = Sandbox::new();
  sb.replay(&[
    ("plugin list --json", "list"),
    ("plugin update ast-grep@toolu --json", "update-updated"),
    ("plugin update jev@toolu --json", "update-up-to-date"),
    ("plugin update toolu@toolu --json", "update-up-to-date"),
  ]);
  let Ok(steps) = run(&driver(&sb), &catalog(), &[], false) else {
    panic!("update failed outright");
  };
  let hint = |name: &str| {
    format!(
      "retired from the toolu marketplace; remove it with `toolu plugins remove {name} --host claude --yes`"
    )
  };
  let seen: Vec<(&str, Status, String)> = steps
    .iter()
    .map(|step| (step.name.as_str(), step.status, step.detail.clone()))
    .collect();
  assert_eq!(
    seen,
    [
      (
        "ast-grep",
        Status::Updated,
        "updated 8.5.0 → 8.6.0".to_owned()
      ),
      ("context7", Status::Skew, hint("context7")),
      ("exa-search", Status::Skew, hint("exa-search")),
      ("jev", Status::Current, "current at 8.5.0".to_owned()),
      ("toolu", Status::Current, "current at 8.5.0".to_owned()),
    ]
  );
  assert_eq!(
    updates(&sb),
    [
      "plugin update ast-grep@toolu --json",
      "plugin update jev@toolu --json",
      "plugin update toolu@toolu --json"
    ]
  );
  assert!(
    !sb
      .log("claude")
      .iter()
      .any(|line| line.contains("--available"))
  );
}

#[test]
fn each_captured_claude_result_line_maps_to_its_step() {
  let cases = [
    ("update-up-to-date", Status::Current, "current at 8.5.0"),
    ("update-updated", Status::Updated, "updated 8.5.0 → 8.6.0"),
    (
      "update-skipped",
      Status::Skew,
      "jev is pinned to 8.5.0; skipped.",
    ),
    (
      "update-refresh-failed",
      Status::Skew,
      "could not refresh the marketplace; still at 8.5.0",
    ),
    (
      "update-not-found",
      Status::Failed,
      "Plugin \"cloudflare\" not found",
    ),
  ];
  for (case, status, detail) in cases {
    let sb = Sandbox::new();
    sb.replay(&[("plugin update jev@toolu --json", case)]);
    let Ok(steps) = run(&driver(&sb), &catalog(), &names(&["jev"]), false) else {
      panic!("update failed outright for {case}");
    };
    assert_eq!(
      (steps[0].status, steps[0].detail.as_str()),
      (status, detail),
      "{case}"
    );
  }
}

#[test]
fn a_named_plugin_the_host_does_not_report_is_still_attempted() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let Ok(steps) = run(&driver(&sb), &catalog(), &names(&["jev"]), false) else {
    panic!("update failed outright");
  };
  assert_eq!(
    (steps[0].status, steps[0].detail.as_str()),
    (Status::Updated, "updated")
  );
  let Ok(dry) = run(&driver(&sb), &catalog(), &names(&["jev"]), true) else {
    panic!("the dry run failed");
  };
  assert_eq!(dry[0].status, Status::Skipped);
}

#[test]
fn nothing_requested_and_nothing_installed_says_so() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let Err(fail) = run(&driver(&sb), &catalog(), &[], false) else {
    panic!("an empty update reported success");
  };
  assert_eq!(
    fail.message,
    "claude reported no installed plugins, so there is nothing to update"
  );
}

#[test]
fn muse_updates_its_own_plugins_and_names_a_changed_hook_for_approval() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v8.0.4"]);
  sb.capture("list", "muse-installed.json");
  let listing =
    std::fs::read_to_string(sb.path("muse-installed.json")).expect("a file the test wrote");
  sb.write(
    "muse-installed.json",
    &listing
      .replace("hooky", "brainstorm")
      .replace("8.0.4", "8.0.3"),
  );
  sb.capture("available", "muse-available.json");
  sb.capture("inspect-modified", "muse-inspect-toolu.json");
  let driver = sb.muse_driver("8.0.4", &origin, Approval::Report);
  let Ok(steps) = run(&driver, &catalog(), &[], false) else {
    panic!("the Muse update failed outright");
  };
  let seen: Vec<(&str, Status, &str)> = steps
    .iter()
    .map(|step| (step.name.as_str(), step.status, step.detail.as_str()))
    .collect();
  assert_eq!(
    seen,
    [(
      "toolu",
      Status::Updated,
      "updated to 8.0.4; awaiting approval: muse plugins approve toolu"
    )]
  );
  let log = sb.log("muse");
  assert!(
    log.contains(&"plugins update toolu --json".to_owned()),
    "{log:?}"
  );
  assert!(
    !log.iter().any(|line| line.contains("update brainstorm")),
    "{log:?}"
  );
  let Ok(named) = run(&driver, &catalog(), &names(&["brainstorm"]), false) else {
    panic!("the named Muse update failed outright");
  };
  assert_eq!(named[0].status, Status::Skew);
}

#[test]
fn a_retired_muse_plugin_from_the_checkout_gets_muses_own_remove_command() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v8.0.4"]);
  sb.capture("list", "muse-installed.json");
  let listing =
    std::fs::read_to_string(sb.path("muse-installed.json")).expect("a file the test wrote");
  sb.write(
    "muse-installed.json",
    &listing
      .replace(
        "src/hooky-marketplace/plugins/hooky",
        ".local/share/toolu/muse-marketplace/plugins/jira",
      )
      .replace("hooky", "jira"),
  );
  sb.capture("available", "muse-available.json");
  let driver = sb.muse_driver("8.0.4", &origin, Approval::Report);
  let Ok(steps) = run(&driver, &catalog(), &[], true) else {
    panic!("the Muse dry update failed outright");
  };
  let jira = steps.iter().find(|step| step.name == "jira");
  assert_eq!(
    jira.map(|step| (step.status, step.detail.as_str())),
    Some((
      Status::Skew,
      "retired from the toolu marketplace; remove it with `muse plugins remove jira`"
    ))
  );
}
