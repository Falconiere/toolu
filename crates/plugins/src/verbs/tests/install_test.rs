use std::time::Duration;

use super::run;
use crate::args::{Approval, Scope};
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
    checkout: None,
    pin: None,
    approval: Approval::Report,
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
  assert_eq!(sb.log("claude"), ["plugin list --json"]);
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

#[test]
fn a_failed_marketplace_add_is_named_on_the_plugin_that_then_fails() {
  let sb = Sandbox::new();
  sb.host(
    "claude",
    "[]",
    "\"plugin marketplace add Falconiere/toolu\"|\"plugin install jev@toolu\"",
  );
  let Ok(steps) = run(
    &driver(&sb, Adapter::Claude),
    &catalog(),
    &names(&["jev"]),
    None,
    false,
  ) else {
    panic!("the install failed outright");
  };
  assert_eq!(
    steps[0].detail,
    "boom: plugin install jev@toolu; the marketplace add also failed: boom: plugin marketplace add Falconiere/toolu"
  );
}

/// The captured `muse plugins list --json`, with the plugin installed from
/// another marketplace renamed to the catalog's `brainstorm`; toolu trusted
/// and ast-grep awaiting review once installed. Returns an approving driver.
fn muse_installed(sb: &Sandbox) -> Driver {
  sb.muse("never");
  let origin = sb.origin(&["v8.0.4"]);
  sb.capture("list", "muse-installed.json");
  let listing =
    std::fs::read_to_string(sb.path("muse-installed.json")).expect("a file the test wrote");
  sb.write(
    "muse-installed.json",
    &listing.replace("hooky", "brainstorm"),
  );
  sb.capture("available", "muse-available.json");
  sb.capture("inspect-toolu-trusted", "muse-inspect-toolu.json");
  sb.capture("inspect-review-needed", "muse-inspect-ast-grep.json");
  sb.muse_driver("8.0.4", &origin, Approval::Approve)
}

/// The commands the `muse` stub ran that change or read trust.
fn muse_writes(sb: &Sandbox) -> Vec<String> {
  sb.log("muse")
    .into_iter()
    .filter(|line| {
      ["install ", "inspect", "approve", "add"]
        .iter()
        .any(|word| line.contains(word))
    })
    .collect()
}

#[test]
fn muse_installs_from_the_checkout_approves_what_is_pending_and_leaves_foreign_plugins() {
  let sb = Sandbox::new();
  let driver = muse_installed(&sb);
  let Ok(steps) = run(
    &driver,
    &catalog(),
    &names(&["toolu", "brainstorm", "ast-grep"]),
    None,
    false,
  ) else {
    panic!("the Muse install failed outright");
  };
  let seen = |name: &str| {
    steps
      .iter()
      .find(|step| step.name == name)
      .map(|step| (step.status, step.detail.clone()))
  };
  assert_eq!(
    seen("toolu"),
    Some((Status::Already, "already installed at 8.0.4".to_owned()))
  );
  assert_eq!(
    seen("ast-grep"),
    Some((Status::Installed, "installed; approved".to_owned()))
  );
  let Some((status, detail)) = seen("brainstorm") else {
    panic!("brainstorm was not reported");
  };
  assert_eq!(status, Status::Skew);
  assert!(
    detail.contains("outside the toolu marketplace")
      && detail.contains("muse plugins remove brainstorm"),
    "{detail}"
  );
  let dir = sb
    .path("home/.local/share/toolu/muse-marketplace")
    .display()
    .to_string();
  assert_eq!(
    muse_writes(&sb),
    [
      format!("plugins marketplace add toolu {dir} --json"),
      "plugins install ast-grep@toolu --json".to_owned(),
      "plugins inspect toolu --json".to_owned(),
      "plugins inspect ast-grep --json".to_owned(),
      "plugins approve ast-grep --json".to_owned(),
    ]
  );
}

#[test]
fn a_muse_checkout_that_cannot_be_made_installs_nothing() {
  let sb = Sandbox::new();
  sb.muse("never");
  let origin = sb.origin(&["v8.0.4"]);
  let driver = sb.muse_driver("0.0.0-dev", &origin, Approval::Approve);
  let failed = run(&driver, &catalog(), &names(&["toolu"]), None, false);
  assert!(failed.is_err_and(|fail| fail.message.contains("v0.0.0-dev")));
  assert_eq!(sb.log("muse"), Vec::<String>::new());
}

#[test]
fn a_muse_dry_run_clones_nothing_and_changes_no_marketplace() {
  let sb = Sandbox::new();
  let driver = muse_installed(&sb);
  let Ok(steps) = run(&driver, &catalog(), &names(&["ast-grep"]), None, true) else {
    panic!("the Muse dry run failed");
  };
  assert_eq!(
    (steps[0].status, steps[0].argv.join(" ")),
    (
      Status::Skipped,
      "muse plugins install ast-grep@toolu --json".to_owned()
    )
  );
  assert!(!sb.path("home/.local/share/toolu/muse-marketplace").exists());
  assert_eq!(
    sb.log("muse"),
    ["plugins list --json", "plugins list --available --json"]
  );
}
