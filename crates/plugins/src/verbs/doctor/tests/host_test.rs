use super::{InventoryFail, setup_findings};
use crate::dispatch::Context;
use crate::host::Target;
use crate::host::adapter::Installed;
use crate::verbs::doctor::{Level, Scope};

fn cx(version: &'static str) -> Context<'static> {
  Context {
    env: toolu_runtime::env::Env::from_pairs([("HOME", "/tmp/toolu-doctor-host-test")]),
    host: None,
    ask: None,
    timeout: std::time::Duration::from_secs(1),
    version,
    origin: String::new(),
    style: toolu_runtime::term::Style::Plain,
    started: std::time::Instant::now(),
    progress: None,
  }
}

#[test]
fn an_empty_inventory_warns_with_the_install_command() {
  let findings = setup_findings(Target::Claude, None, &[], &cx("8.6.0"));
  assert_eq!(findings.len(), 1);
  assert_eq!(findings[0].status, Level::Warn);
  assert_eq!(findings[0].scope, Scope::Host);
  assert!(findings[0].summary.contains("no toolu plugin"));
  assert_eq!(
    findings[0].fix.as_deref(),
    Some("toolu plugins install --host claude")
  );
}

#[test]
fn a_list_failure_is_a_host_fail_without_stderr() {
  let findings = setup_findings(
    Target::Claude,
    Some(&InventoryFail::ListFailed),
    &[],
    &cx("8.6.0"),
  );
  assert_eq!(findings.len(), 1);
  assert_eq!(findings[0].status, Level::Fail);
  assert_eq!(
    findings[0].summary,
    "could not list its plugins (host command failed)"
  );
  assert_eq!(
    findings[0].fix.as_deref(),
    Some("claude plugin list --json")
  );
}

#[test]
fn a_driver_failure_carries_the_message_and_skips_the_empty_warn() {
  let findings = setup_findings(
    Target::Muse,
    Some(&InventoryFail::Driver("muse is not on PATH".into())),
    &[],
    &cx("8.6.0"),
  );
  assert_eq!(findings.len(), 1);
  assert_eq!(findings[0].status, Level::Fail);
  assert_eq!(findings[0].summary, "muse is not on PATH");
  assert!(findings[0].fix.is_none());
}

#[test]
fn an_installed_plugin_suppresses_the_empty_inventory_warn() {
  let installed = [Installed {
    name: "jev".into(),
    version: "8.6.0".into(),
    enabled: true,
    source: None,
  }];
  let findings = setup_findings(Target::Claude, None, &installed, &cx("8.6.0"));
  assert!(findings.is_empty(), "{findings:?}");
}
