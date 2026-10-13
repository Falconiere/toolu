use std::time::Duration;

use super::run;
use crate::host::adapter::Adapter;
use crate::support::{Sandbox, catalog};
use crate::verbs::Driver;

#[test]
fn the_catalog_is_joined_with_the_hosts_listing() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write("claude-installed.json", r#"[{"id":"jev@toolu","version":"7.11.0","enabled":false}]"#);
  let driver = Driver { adapter: Adapter::Claude, env: sb.env(), timeout: Duration::from_secs(10) };
  let Ok(rows) = run(&driver, &catalog()) else {
    panic!("list failed");
  };
  assert_eq!(rows.len(), catalog().plugins.len());
  let jev = rows.iter().find(|row| row.name == "jev");
  assert_eq!(jev.map(|row| (row.installed, row.version.as_deref(), row.enabled)), Some((true, Some("7.11.0"), false)));
  let toolu = rows.iter().find(|row| row.name == "toolu");
  assert_eq!(toolu.map(|row| (row.installed, row.version.is_none())), Some((false, true)));
}

#[test]
fn a_malformed_listing_fails_and_a_failing_host_lists_nothing_installed() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write("claude-installed.json", r#"{"installed":[]}"#);
  let driver = Driver { adapter: Adapter::Claude, env: sb.env(), timeout: Duration::from_secs(10) };
  assert!(run(&driver, &catalog()).is_err_and(|fail| fail.message.starts_with("claude plugin list --json: ")));
  let codex = Driver { adapter: Adapter::Codex, ..driver };
  let rows = run(&codex, &catalog()).unwrap_or_default();
  assert!(!rows.is_empty() && rows.iter().all(|row| !row.installed));
}
