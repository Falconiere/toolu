use std::net::SocketAddr;

use serde_json::{Value, json};

use super::{build, dashboard};
use crate::logic::ensure_issue;
use crate::model::World;

#[test]
fn a_notice_carries_the_issue_snapshot_and_dashboard() {
  let tmp = tempfile::tempdir().expect("temp");
  let state_dir = tmp.path().display().to_string();
  std::fs::create_dir_all(tmp.path().join("issues")).expect("issues");
  std::fs::write(
    tmp.path().join("issues/a.json"),
    json!({"title": "Attention queue", "url": "https://github.com/o/r/issues/450"}).to_string(),
  )
  .expect("snapshot");
  let mut world = World::new(1_000);
  ensure_issue(&mut world, "a", "falconiere-toolu-402", &state_dir);
  let token = crate::attention::raise(&mut world, "needs-human", "a", &"x".repeat(300));
  world.now_ms = 66_000;
  let notice = build(&world, &token, true, Some("http://127.0.0.1:7717/")).expect("notice");
  assert_eq!(notice.title, "toolu epic: a needs-human");
  assert_eq!(notice.body.chars().count(), 200);
  assert_eq!(notice.key, "a");
  let payload = &notice.payload;
  assert_eq!(payload["epic"], "falconiere-toolu-402");
  assert_eq!(payload["state"], "open");
  assert_eq!(payload["age_seconds"], 65);
  assert_eq!(payload["reminder"], true);
  assert_eq!(
    payload["issue"],
    json!({"title": "Attention queue", "url": "https://github.com/o/r/issues/450"})
  );
  let text = payload["text"].as_str().expect("text");
  assert!(
    text.starts_with("toolu epic: a needs-human (reminder): xxx"),
    "{text}"
  );
  assert!(text.ends_with(" https://github.com/o/r/issues/450 http://127.0.0.1:7717/"));
}

#[test]
fn a_notice_without_a_snapshot_or_dashboard_has_nulls_and_a_closed_item_has_none() {
  let mut world = World::new(0);
  let token = crate::attention::raise(&mut world, "dependency-cycle", "", "dependency cycle");
  let notice = build(&world, &token, false, None).expect("notice");
  assert_eq!(notice.payload["issue"], Value::Null);
  assert_eq!(notice.payload["dashboard"], Value::Null);
  assert_eq!(
    notice.payload["text"],
    "toolu epic:  dependency-cycle: dependency cycle"
  );
  assert_eq!(build(&world, "a0-99", false, None), None);
}

#[test]
fn dashboard_shows_an_unspecified_bind_as_loopback() {
  let cases = [
    ("0.0.0.0:7717", "http://127.0.0.1:7717/"),
    ("[::]:7717", "http://127.0.0.1:7717/"),
    ("192.168.1.4:80", "http://192.168.1.4:80/"),
    ("[::1]:9000", "http://[::1]:9000/"),
  ];
  for (addr, url) in cases {
    let addr: SocketAddr = addr.parse().expect("addr");
    assert_eq!(dashboard(addr), url);
  }
}
