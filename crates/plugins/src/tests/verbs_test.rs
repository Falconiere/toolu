use std::time::Duration;

use super::{Driver, find};
use crate::args::Approval;
use crate::host::adapter::Adapter;
use crate::support::Sandbox;

#[test]
fn a_listing_that_fails_or_is_malformed_reads_as_empty() {
  let sb = Sandbox::new();
  sb.host("claude", "[]", "never");
  sb.write(
    "claude-installed.json",
    r#"[{"id":"jev@toolu","version":"7.11.0"}]"#,
  );
  let driver = Driver {
    adapter: Adapter::Claude,
    env: sb.env(),
    timeout: Duration::from_secs(10),
    checkout: None,
    pin: None,
    approval: Approval::Report,
  };
  let installed = driver.versions(&driver.adapter.list_installed());
  assert_eq!(
    find(&installed, "jev").map(|entry| entry.version.as_str()),
    Some("7.11.0")
  );
  assert_eq!(find(&installed, "toolu"), None);
  assert_eq!(driver.adapter.list_available(), None);
  let gone = Driver {
    adapter: Adapter::Codex,
    ..driver
  };
  assert_eq!(gone.versions(&gone.adapter.list_installed()), Vec::new());
}
