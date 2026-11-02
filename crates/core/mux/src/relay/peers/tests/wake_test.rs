use super::{Wake, claim, eligible};
use crate::relay::peers::tests::{SECRET, auth, envelope, join, request, send, session, setup};
use serde_json::json;

#[test]
fn busy_candidates_do_not_starve_later_unread_notices() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice");
  join(root, "bob");
  for index in 0..5 {
    send(
      root,
      "alice",
      envelope("alice", "bob", &format!("m{index}")),
    );
  }
  super::storage::edit(root, "foundation", |ledger| {
    for message in ledger.messages.values_mut() {
      message.wake = "pending".into();
    }
    let cfg = super::config::load(root).unwrap();
    let first = super::candidates(ledger, &cfg);
    let second = super::candidates(ledger, &cfg);
    assert_eq!(first.len(), 4);
    assert_eq!(second.len(), 4);
    assert!(!first.iter().any(|(key, _)| key == "alice/m4"));
    assert!(second.iter().any(|(key, _)| key == "alice/m4"));
    Ok(())
  })
  .unwrap();
}

#[test]
fn wake_requires_idle_exact_terminal_incarnation_and_owner() {
  let wake = Wake {
    address: "one/t7".into(),
    owner: "alice:gen".into(),
    created_ms: 123,
    pid: 42,
  };
  let terminal = json!({"address":"one/t7","created_ms":123,"foreground":{"pid":42},"labels":{"toolu.peers":"alice:gen"},"state":"live","status":{"state":"idle"}});
  assert!(eligible(&wake, &terminal));
  let mut replacement = terminal.clone();
  replacement["foreground"]["pid"] = json!(43);
  assert!(!eligible(&wake, &replacement));
  replacement["foreground"] = json!({});
  assert!(!eligible(&wake, &replacement));
  let legacy: Wake =
    serde_json::from_value(json!({"address":"one/t7","owner":"alice:gen","created_ms":123}))
      .unwrap();
  assert_eq!(legacy.pid, 0);
  assert!(!eligible(&legacy, &terminal));
  for state in ["working", "blocked", "unknown", "done", "error"] {
    let mut changed = terminal.clone();
    changed["status"]["state"] = json!(state);
    assert!(!eligible(&wake, &changed));
  }
  for (key, value) in [
    ("address", json!("one/t8")),
    ("created_ms", json!(124)),
    ("state", json!("finished")),
    ("labels", json!({})),
  ] {
    let mut changed = terminal.clone();
    changed[key] = value;
    assert!(!eligible(&wake, &changed));
  }
}

#[test]
fn wake_intent_is_durable_and_never_replayed_after_uncertain_send() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  setup(root);
  join(root, "alice");
  let mut req = request("join");
  let mut bob = session("bob");
  bob["wake"] = json!({"address":"one/t7","owner":"bob:gen","created_ms":123,"pid":42});
  req["session"] = bob;
  req["secret"] = json!(SECRET);
  req["group_key"] = json!(SECRET);
  assert!(
    crate::relay::peers::receive(root, &req)
      .get("result")
      .is_some()
  );
  assert_eq!(
    send(root, "alice", envelope("alice", "bob", "m1"))["result"]["wake"],
    "pending"
  );
  assert!(claim(root, "foundation", "alice/m1").unwrap());
  assert!(!claim(root, "foundation", "alice/m1").unwrap());
  assert_eq!(
    send(root, "alice", envelope("alice", "bob", "m2"))["result"]["wake"],
    "pending"
  );
  let mut ack = request("ack");
  ack["auth"] = auth("bob");
  ack["key"] = json!("alice/m2");
  crate::relay::peers::receive(root, &ack);
  assert!(!claim(root, "foundation", "alice/m2").unwrap());
}
