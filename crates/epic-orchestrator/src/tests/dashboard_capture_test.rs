//! The dashboard fixture: every document the page reads, captured from running engines.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;
use toolu_state::time::epoch_millis;

use crate::disk::write_value;
use crate::fleet_rig::{Engine, config, free_port, get_json, secrets_of};

fn save_capture(captured: &Value) {
  if std::env::var_os("TOOLU_CAPTURE_DASHBOARD").is_none() {
    return;
  }
  let path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/dashboard/status.json");
  std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture dir");
  std::fs::write(
    &path,
    serde_json::to_vec_pretty(captured).expect("fixture json"),
  )
  .expect("write fixture");
}

const TOKEN: &str = "dashboard-capture-token";

/// The status code and JSON body of a worker screen read.
fn screen(addr: std::net::SocketAddr, headers: &str) -> Value {
  let mut stream = TcpStream::connect(addr).expect("connect");
  write!(
    stream,
    "GET /api/v1/issues/local-658/screen HTTP/1.1\r\nHost: {addr}\r\n{headers}Connection: close\r\n\r\n"
  )
  .expect("request");
  let mut response = String::new();
  stream.read_to_string(&mut response).expect("response");
  let (head, body) = response.split_once("\r\n\r\n").expect("header end");
  let status: u16 = head
    .get(9..12)
    .and_then(|code| code.parse().ok())
    .expect("status code");
  json!({"status": status, "body": serde_json::from_str::<Value>(body).expect("json")})
}

fn seed_host_states(root: &Path) {
  let now = epoch_millis(std::time::SystemTime::now());
  write_value(
    &root.join("state.json"),
    &json!({
      "version": 1,
      "leases": [],
      "cooldowns": {"codex": {"until": now + 41 * 60_000, "reason": "usage limit"}},
      "paused_hosts": {"muse": {"by": "user", "at": now}}
    }),
  )
  .expect("host states");
}

/// A Claude Code worktree on branch `feat/fixture` holding the committed copy of a
/// real branch ledger, so the capture never depends on the checkout's own branch.
fn seed_issue_record(local: &Engine) {
  let worktree = local.root.join("worktree");
  std::fs::create_dir_all(worktree.join(".git")).expect("git dir");
  std::fs::write(worktree.join(".git/HEAD"), "ref: refs/heads/feat/fixture\n").expect("HEAD");
  let ledgers = worktree.join(".claude/tmp/plan-ledger");
  std::fs::create_dir_all(&ledgers).expect("ledger dir");
  let fixture =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/dashboard/ledger.json");
  std::fs::copy(fixture, ledgers.join("feat_fixture.json")).expect("ledger fixture");
  let issue_dir = local.root.join("local-649/issues");
  std::fs::create_dir_all(&issue_dir).expect("issue dir");
  write_value(
    &issue_dir.join("local-658.json"),
    &json!({"key": "local-658", "kind": "claude", "worktree": worktree}),
  )
  .expect("issue record");
}

/// Every document the dashboard reads, from the running engines.
fn capture(local: &Engine, peer: &Engine) -> Value {
  json!({
    "overview": get_json(local.addr, "/api/v1/overview"),
    "epics": get_json(local.addr, "/api/v1/epics"),
    "board": get_json(local.addr, "/api/v1/epics/local-649"),
    "hosts": get_json(local.addr, "/api/v1/hosts"),
    "attention": get_json(local.addr, "/api/v1/attention"),
    "fleet": get_json(local.addr, "/api/v1/fleet"),
    "events": get_json(local.addr, "/api/v1/events?limit=7"),
    "live": get_json(local.addr, "/api/v1/agents/live"),
    "peer_overview": get_json(local.addr, "/api/v1/fleet/peers/build-mini/overview"),
    "peer_board": get_json(local.addr, "/api/v1/fleet/peers/build-mini/epics/peer-649"),
    "issue": get_json(local.addr, "/api/v1/issues/local-658"),
    "peer_issue": get_json(local.addr, "/api/v1/fleet/peers/build-mini/issues/peer-658"),
    "screen": screen(local.addr, &format!("Authorization: Bearer {TOKEN}\r\n")),
    "screen_locked": screen(local.addr, ""),
    "screen_disabled": screen(peer.addr, "")
  })
}

#[test]
fn dashboard_status_capture_comes_from_running_engines() {
  let temp = tempfile::tempdir().expect("temp root");
  let peer = Engine::start_titled(
    &temp.path().join("peer"),
    ("peer-649", "Peer dashboard"),
    "peer-658",
    config(free_port(), Vec::new(), Secrets::none()),
  );
  let local = Engine::start_titled(
    &temp.path().join("local"),
    ("local-649", "Dashboard redesign"),
    "local-658",
    config(
      free_port(),
      vec![peer.peer("build-mini")],
      secrets_of(temp.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]),
    ),
  );
  seed_host_states(&local.paths.root);
  seed_issue_record(&local);
  let captured = capture(&local, &peer);
  assert_eq!(captured["issue"]["key"], "local-658");
  assert_eq!(captured["issue"]["plan"]["summary"]["total"], 7);
  assert_eq!(captured["screen_locked"]["status"], 401);
  assert_eq!(captured["screen_disabled"]["status"], 403);
  assert_eq!(captured["board"]["issues"][0]["key"], "local-658");
  assert_eq!(captured["peer_board"]["issues"][0]["key"], "peer-658");
  assert_eq!(captured["fleet"]["machines"][1]["name"], "build-mini");
  assert_eq!(captured["hosts"]["items"][1]["reason"], "usage limit");
  assert_eq!(captured["hosts"]["items"][3]["paused"], true);
  save_capture(&captured);
  local.stop();
  peer.stop();
}
