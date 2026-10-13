use std::time::Duration;

use super::run;
use crate::args::Scope;
use crate::catalog::order::dependents_of;
use crate::host::adapter::Adapter;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};
use crate::verbs::Driver;

fn driver(sb: &Sandbox, adapter: Adapter) -> Driver {
  Driver {
    adapter,
    env: sb.env(),
    timeout: Duration::from_secs(10),
  }
}

#[test]
fn a_dry_run_orders_every_dependent_after_the_core_and_runs_no_install() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  let catalog = catalog();
  let Ok(steps) = run(
    &driver(&sb, Adapter::Claude),
    &catalog,
    &[],
    Some(Scope::User),
    true,
  ) else {
    panic!("the dry run failed");
  };
  assert_eq!(steps.len(), catalog.plugins.len());
  let core = steps.iter().position(|step| step.name == "toolu");
  for dependent in dependents_of(&catalog, "toolu") {
    assert!(
      steps.iter().position(|step| step.name == dependent) > core,
      "{dependent}"
    );
  }
  assert!(
    steps
      .iter()
      .all(|step| step.status == Status::Skipped && step.detail == "dry run")
  );
  assert!(
    steps
      .iter()
      .all(|step| step.argv.contains(&"--scope".to_owned()))
  );
  assert_eq!(
    sb.log("claude"),
    ["plugin list --json", "plugin list --available --json"]
  );
}

#[test]
fn a_core_failure_skips_its_dependents_and_others_still_install() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "\"plugin install toolu@toolu\"");
  let request = names(&["toolu", "ts-quality", "jev"]);
  let Ok(steps) = run(
    &driver(&sb, Adapter::Claude),
    &catalog(),
    &request,
    None,
    false,
  ) else {
    panic!("the install failed outright");
  };
  let seen: Vec<(&str, Status, &str)> = steps
    .iter()
    .map(|step| (step.name.as_str(), step.status, step.detail.as_str()))
    .collect();
  assert_eq!(
    seen,
    [
      ("toolu", Status::Failed, "boom: plugin install toolu@toolu"),
      ("ts-quality", Status::Skipped, "skipped: toolu failed"),
      ("jev", Status::Installed, "installed"),
    ]
  );
  let log = sb.log("claude");
  assert_eq!(
    log.first().map(String::as_str),
    Some("plugin marketplace add Falconiere/toolu")
  );
  assert!(
    !log.iter().any(|line| line.contains("ts-quality")),
    "{log:?}"
  );
}

#[test]
fn an_installed_plugin_is_already_there_or_skewed_and_left_alone() {
  let sb = Sandbox::new();
  sb.host("codex", r#"{"installed":[]}"#, "never");
  sb.write(
    "codex-installed.json",
    r#"{"installed":[{"name":"jev","version":"7.10.0"},{"name":"ast-grep","version":"7.11.0"}]}"#,
  );
  sb.write(
    "codex-available.json",
    r#"{"installed":[{"name":"jev","version":"7.11.0"},{"name":"ast-grep","version":"7.11.0"}]}"#,
  );
  let Ok(steps) = run(
    &driver(&sb, Adapter::Codex),
    &catalog(),
    &names(&["jev", "ast-grep"]),
    None,
    false,
  ) else {
    panic!("the install failed outright");
  };
  assert_eq!(steps[0].status, Status::Skew);
  assert_eq!(
    steps[0].detail,
    "installed at 7.10.0, marketplace offers 7.11.0; left untouched. Run `toolu plugins update jev` to change it."
  );
  assert_eq!(
    (steps[1].status, steps[1].detail.as_str()),
    (Status::Already, "already installed at 7.11.0")
  );
  assert!(
    !sb
      .log("codex")
      .iter()
      .any(|line| line.starts_with("plugin add"))
  );
}

#[test]
fn a_missing_host_fails_the_core_and_an_unknown_name_runs_nothing() {
  let sb = Sandbox::new();
  let Ok(steps) = run(
    &driver(&sb, Adapter::Claude),
    &catalog(),
    &names(&["rust-quality"]),
    None,
    false,
  ) else {
    panic!("the install failed outright");
  };
  assert_eq!(
    steps.iter().map(|step| step.status).collect::<Vec<_>>(),
    [Status::Failed, Status::Skipped]
  );
  sb.host("claude", "[]", "never");
  let unknown = run(
    &driver(&sb, Adapter::Claude),
    &catalog(),
    &names(&["not-a-plugin"]),
    None,
    false,
  );
  assert!(unknown.is_err_and(|fail| fail.message.starts_with("unknown plugin: not-a-plugin")));
  assert_eq!(sb.log("claude"), Vec::<String>::new());
}
