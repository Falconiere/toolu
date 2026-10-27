//! Fleet fields the dashboard reads: machine names, peer dashboard URLs and
//! the epic board through the control socket, on real engines.

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use crate::fleet_rig::{Engine, config, free_port, get_json, machine};
use crate::peer_fixture::peer;

/// This machine's `hostname` cut at the first `.`.
fn short_hostname() -> String {
  let output = std::process::Command::new("hostname")
    .output()
    .expect("hostname");
  let name = String::from_utf8(output.stdout).expect("utf-8");
  name.trim().split('.').next().unwrap_or("").to_owned()
}

#[test]
fn fleet_machine_names_and_dashboard_urls() {
  let temp = tempfile::tempdir().expect("temp root");
  let b = Engine::start(
    &temp.path().join("b"),
    "repo-500",
    "repo-501",
    config(free_port(), Vec::new(), Secrets::none()),
  );
  let down = crate::peer_fixture::closed();
  let peers = vec![b.peer("server"), peer("down", down)];
  let a = Engine::start(
    &temp.path().join("a"),
    "repo-402",
    "repo-449",
    config(free_port(), peers, Secrets::none()),
  );
  let host = short_hostname();
  assert_eq!(
    get_json(a.addr, "/api/v1/overview")["hostname"],
    json!(host)
  );
  let fleet = get_json(a.addr, "/api/v1/fleet");
  let fields = |name: &str| {
    let found = machine(&fleet, name);
    json!([found["state"], found["hostname"], found["dashboard_url"]])
  };
  assert_eq!(fields("local"), json!(["ok", host, null]));
  assert_eq!(
    fields("server"),
    json!(["ok", host, format!("http://{}/", b.addr)])
  );
  assert_eq!(
    fields("down"),
    json!(["unreachable", null, format!("http://{down}/")])
  );
  a.stop();
  b.stop();
}

#[test]
fn fleet_socket_status_matches_the_epic_board() {
  let temp = tempfile::tempdir().expect("temp root");
  let a_config = config(free_port(), Vec::new(), Secrets::none());
  let root = temp.path().join("a");
  let a = Engine::start_titled(&root, ("repo-402", "Fleet epic"), "repo-449", a_config);
  let request = json!({"op": "status", "epic": "repo-402"});
  let socket = crate::client::exchange_retry(&a.paths, crate::PROTOCOL, &request).expect("status");
  let http = get_json(a.addr, "/api/v1/epics/repo-402");
  let header = |doc: &Value| {
    json!([
      doc["title"],
      doc["url"],
      doc["waves_total"],
      doc["current_wave"],
      doc["live"],
      doc["queued"]
    ])
  };
  assert_eq!(
    header(&http),
    json!(["Fleet epic", "https://github.com/o/r/issues/9", 1, 1, 1, 0])
  );
  assert_eq!(header(&socket), header(&http));
  let row = |doc: &Value| {
    let row = &doc["issues"][0];
    assert!(row["phase_age_ms"].is_u64(), "{row}");
    json!([row["key"], row["model"], row["effort"], row["hold"]])
  };
  assert_eq!(row(&socket), row(&http));
  a.stop();
}
