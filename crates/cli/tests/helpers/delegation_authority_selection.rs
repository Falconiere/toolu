use super::{Fixture, active, ok, pending_return, register, setup};
use serde_json::{Value, json};

#[test]
fn accepted_pending_enrollment_can_refresh_after_another_migration() {
  let fixture = Fixture::new().unwrap();
  let (expected, next) = pending_return(&fixture, "lost-generation").unwrap();
  ok(
    fixture
      .rpc(
        "alpha",
        &json!({"op":"enroll", "expected":expected, "config":next}),
      )
      .unwrap(),
  )
  .unwrap();
  setup(&fixture, "b", "alpha").unwrap();
  register(&fixture, "b", "fresh-worker").unwrap();
  setup(&fixture, "b", "beta").unwrap();
  let result = fixture
    .cli("a", &["setup", "relay", "--machine", "alpha"])
    .unwrap();
  assert!(!result.status.success());
  assert!(
    String::from_utf8_lossy(&result.stdout).contains("beta"),
    "stdout: {} stderr: {}",
    String::from_utf8_lossy(&result.stdout),
    String::from_utf8_lossy(&result.stderr)
  );
  assert!(
    !fixture
      .controller_root("a")
      .join("delegation/pending-relay.json")
      .exists()
  );
  setup(&fixture, "a", "beta").unwrap();
  assert!(
    active(&fixture, "beta").unwrap()["machines"]
      .get("fresh-worker")
      .is_some()
  );
  let local: Value = serde_json::from_slice(
    &std::fs::read(fixture.controller_root("a").join("delegation/config.json")).unwrap(),
  )
  .unwrap();
  assert!(local["machines"].get("fresh-worker").is_some());
}
