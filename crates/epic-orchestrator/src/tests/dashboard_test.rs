use toolu_runtime::config::secrets::Secrets;

use crate::fleet_rig::{config, get, listener};

#[test]
fn fleet_dashboard() {
  let temp = tempfile::tempdir().expect("temp root");
  let (running, stop) = listener(temp.path(), config(0, Vec::new(), Secrets::none()));
  let (status, page) = get(running.addr, "/");
  assert_eq!(status, 200);
  for needle in [
    r#"<dl id="fleet-summary" class="summary" hidden>"#,
    r#"<div id="machines" class="tabs machines" role="tablist" aria-label="Machines" hidden>"#,
    r#"id="machine-state""#,
    r#"id="panels""#,
    "get('/api/v1/fleet')",
    "`/api/v1/fleet/peers/${encodeURIComponent(state.machine)}`",
    "Unreachable · never seen",
    "Unauthorized: token rejected",
    "setInterval(",
  ] {
    assert!(page.contains(needle), "missing {needle}");
  }
  assert!(!page.contains("http://") && !page.contains("https://"));
  running.stop(&stop).expect("listener thread");
}
