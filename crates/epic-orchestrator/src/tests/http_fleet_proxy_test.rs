use std::thread;
use std::time::Duration;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use crate::fleet_rig::{Engine, config, free_port, get, get_json, listener, secrets_of};
use crate::peer_fixture::{blackhole, peer, serve};

#[test]
fn fleet_proxy_documents() {
  let temp = tempfile::tempdir().expect("temp root");
  let b_config = config(free_port(), Vec::new(), Secrets::none());
  let b = Engine::start(&temp.path().join("b"), "repo-500", "repo-501", b_config);
  let peers = vec![b.peer("server"), b.peer("my server/1")];
  let (a, stop) = listener(&temp.path().join("a"), config(0, peers, Secrets::none()));
  let epic = get_json(a.addr, "/api/v1/fleet/peers/server/epics/repo-500");
  assert_eq!(epic["key"], "repo-500");
  assert_eq!(epic["graph"]["issues"][0]["key"], "repo-501");
  let issue = get_json(a.addr, "/api/v1/fleet/peers/server/issues/repo-501");
  assert_eq!(
    (issue["key"].clone(), issue["stage"].clone()),
    (json!("repo-501"), json!("running"))
  );
  let encoded = get_json(a.addr, "/api/v1/fleet/peers/my%20server%2F1/overview?x=1");
  assert_eq!(encoded["schema"], 1);
  let (missing, _) = get(a.addr, "/api/v1/fleet/peers/server/issues/repo-999");
  assert_eq!(missing, 404);
  a.stop(&stop).expect("listener thread");
  b.stop();
}

#[test]
fn fleet_proxy_rejects_paths() {
  let temp = tempfile::tempdir().expect("temp root");
  let any = serve(200, "application/json", r#"{"schema":1,"ok":true}"#);
  let refused = serve(
    401,
    "application/json",
    r#"{"schema":1,"error":"unauthorized"}"#,
  );
  let old = serve(200, "application/json", r#"{"schema":2}"#);
  let peers = vec![any.peer("any"), refused.peer("refused"), old.peer("old")];
  let (a, stop) = listener(temp.path(), config(0, peers, Secrets::none()));
  let long = "k".repeat(128);
  assert_eq!(
    get_json(a.addr, &format!("/api/v1/fleet/peers/any/issues/{long}"))["ok"],
    true
  );
  let before = any.seen();
  for path in [
    "/api/v1/fleet/peers/nobody/overview".to_owned(),
    "/api/v1/fleet/peers/any/metrics".to_owned(),
    "/api/v1/fleet/peers/any/events/stream".to_owned(),
    "/api/v1/fleet/peers/any/issues/..".to_owned(),
    "/api/v1/fleet/peers/any/issues/a%2Fb".to_owned(),
    format!("/api/v1/fleet/peers/any/issues/{long}k"),
    "/api/v1/fleet/peers/any".to_owned(),
    "/api/v1/fleet/peers/%zz/overview".to_owned(),
    "/api/v1/fleet/other".to_owned(),
  ] {
    let (status, body) = get(a.addr, &path);
    assert_eq!(
      (status, body.as_str()),
      (404, r#"{"error":"not_found","schema":1}"#),
      "{path}"
    );
  }
  assert_eq!(any.seen(), before);
  let (status, body) = get(a.addr, "/api/v1/fleet/peers/refused/overview");
  assert_eq!(
    (status, body.contains("peer_unauthorized")),
    (502, true),
    "{body}"
  );
  let (status, body) = get(a.addr, "/api/v1/fleet/peers/old/queue");
  assert_eq!(
    (status, body.contains("peer_version_mismatch")),
    (502, true),
    "{body}"
  );
  a.stop(&stop).expect("listener thread");
}

#[test]
fn fleet_proxy_limit() {
  let temp = tempfile::tempdir().expect("temp root");
  let hole = blackhole();
  let peers = vec![peer("hole", hole.local_addr().expect("hole"))];
  let (a, stop) = listener(temp.path(), config(0, peers, Secrets::none()));
  let addr = a.addr;
  let waiting: Vec<_> = (0..8)
    .map(|_| thread::spawn(move || get(addr, "/api/v1/fleet/peers/hole/overview")))
    .collect();
  thread::sleep(Duration::from_millis(300));
  let (status, body) = get(addr, "/api/v1/fleet/peers/hole/hosts");
  assert_eq!(
    (status, body.contains("proxy_limit")),
    (503, true),
    "{body}"
  );
  assert_eq!(get(addr, "/api/v1/overview").0, 200);
  for request in waiting {
    let (status, body) = request.join().expect("proxy call");
    assert_eq!(
      (status, body.contains("peer_unreachable")),
      (502, true),
      "{body}"
    );
  }
  a.stop(&stop).expect("listener thread");
}

#[test]
fn fleet_token_canary_proxy() {
  let temp = tempfile::tempdir().expect("temp root");
  let canary = "PEER-TOKEN-CANARY-2";
  let document = json!({"schema": 1, "key": canary, "items": [{"note": canary}]});
  let echo = serve(200, "application/json", &document.to_string());
  let tokens = json!({"echo": canary}).to_string();
  let secrets = secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let (a, stop) = listener(temp.path(), config(0, vec![echo.peer("echo")], secrets));
  let (status, body) = get(a.addr, "/api/v1/fleet/peers/echo/attention");
  assert_eq!(status, 200);
  assert!(!body.contains(canary), "{body}");
  assert!(body.contains("<redacted>"), "{body}");
  let heads = echo.seen().to_ascii_lowercase();
  assert!(heads.contains("get /api/v1/attention http/1.1"), "{heads}");
  assert!(heads.contains(&format!(
    "authorization: bearer {}",
    canary.to_ascii_lowercase()
  )));
  a.stop(&stop).expect("listener thread");
}
