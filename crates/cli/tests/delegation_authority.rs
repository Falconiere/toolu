//! Authority changes across real native CLI processes and isolated HC sessions.

#[path = "helpers/delegation_authority.rs"]
mod fixture;

use fixture::{Fixture, ok};
use serde_json::{Value, json};

fn setup(fixture: &Fixture, controller: &str, relay: &str) -> Result<(), String> {
  ok(fixture.cli(controller, &["setup", "relay", "--machine", relay])?).map(|_| ())
}

fn register(fixture: &Fixture, controller: &str, worker: &str) -> Result<(), String> {
  ok(fixture.cli(
    controller,
    &[
      "setup",
      "machines",
      "register",
      worker,
      "--root",
      "/srv/worker",
      "--agent",
      "codex",
    ],
  )?)
  .map(|_| ())
}

fn active(fixture: &Fixture, relay: &str) -> Result<Value, String> {
  fixture
    .config(relay)?
    .get("config")
    .cloned()
    .ok_or_else(|| "missing config".into())
}

fn pending_return(fixture: &Fixture, generation: &str) -> Result<(Value, Value), String> {
  setup(fixture, "a", "alpha")?;
  setup(fixture, "a", "beta")?;
  let source = active(fixture, "beta")?;
  let destination = active(fixture, "alpha")?;
  let mut next = source.clone();
  let fields = next.as_object_mut().ok_or("invalid source config")?;
  fields.insert("relay".into(), json!("alpha"));
  fields.insert("generation".into(), json!(generation));
  let pending = json!({"source":{"machine":"beta", "expected":source},
    "destination":"alpha", "expected_destination":destination, "next":next});
  let path = fixture
    .controller_root("a")
    .join("delegation/pending-relay.json");
  if !toolu_state::io::write_atomic(&path, &pending.to_string()) {
    return Err("could not persist pending selection".into());
  }
  ok(fixture.rpc(
    "beta",
    &json!({"op":"retire", "expected":source, "successor":"alpha"}),
  )?)?;
  Ok((destination, next))
}

fn assert_resumed(fixture: &Fixture, generation: &str) -> Result<(), String> {
  setup(fixture, "a", "alpha")?;
  if active(fixture, "alpha")?
    .get("generation")
    .and_then(Value::as_str)
    != Some(generation)
    || !fixture.authority_path("alpha").exists()
    || fixture
      .controller_root("a")
      .join("delegation/pending-relay.json")
      .exists()
  {
    return Err("selection did not resume the saved generation".into());
  }
  Ok(())
}

#[test]
fn stale_controller_edits_merge_with_authority_and_stale_cas_is_rejected() {
  let fixture = Fixture::new().unwrap();
  setup(&fixture, "a", "alpha").unwrap();
  setup(&fixture, "b", "alpha").unwrap();
  let stale = active(&fixture, "alpha").unwrap();
  register(&fixture, "a", "worker-x").unwrap();
  register(&fixture, "b", "worker-y").unwrap();
  let current = active(&fixture, "alpha").unwrap();
  assert!(current["machines"].get("worker-x").is_some());
  assert!(current["machines"].get("worker-y").is_some());
  let stale_update = fixture
    .rpc(
      "alpha",
      &json!({"op":"config", "expected":stale, "config":stale}),
    )
    .unwrap();
  assert!(!stale_update.status.success(), "stale CAS was accepted");
  assert_eq!(active(&fixture, "alpha").unwrap(), current);
}

#[test]
fn retired_authority_cannot_be_reactivated_by_a_stale_controller() {
  let fixture = Fixture::new().unwrap();
  setup(&fixture, "a", "alpha").unwrap();
  setup(&fixture, "b", "alpha").unwrap();
  let old = active(&fixture, "alpha").unwrap();
  let old_generation = old["generation"].as_str().unwrap().to_owned();
  setup(&fixture, "b", "beta").unwrap();
  assert!(register(&fixture, "a", "stale").is_err());
  assert!(
    !fixture
      .cli("a", &["relay", "recover"])
      .unwrap()
      .status
      .success()
  );
  assert!(!fixture.authority_path("alpha").exists());
  assert!(fixture.config("alpha").unwrap()["retirement"].is_object());
  setup(&fixture, "a", "beta").unwrap();
  setup(&fixture, "b", "alpha").unwrap();
  let current = active(&fixture, "alpha").unwrap();
  assert_ne!(current["generation"], old_generation);
  let denied = fixture
    .rpc(
      "alpha",
      &json!({"op":"enroll", "expected":current, "config":old}),
    )
    .unwrap();
  assert!(!denied.status.success(), "retired generation was accepted");
  assert_eq!(active(&fixture, "alpha").unwrap(), current);
  setup(&fixture, "b", "alpha").unwrap();
  assert_eq!(active(&fixture, "alpha").unwrap(), current);
}

#[test]
fn queued_submission_is_idempotent_and_cancel_receipt_survives_migration() {
  let fixture = Fixture::new().unwrap();
  setup(&fixture, "a", "alpha").unwrap();
  let (repo, brief) = fixture.repo().unwrap();
  let submit = [
    "delegate", "submit", "--key", "queued", "--repo", &repo, "--brief", &brief, "--agent", "codex",
  ];
  let first = ok(fixture.cli("a", &submit).unwrap()).unwrap();
  let first: Value = serde_json::from_slice(&first.stdout).unwrap();
  assert_eq!(first["phase"], "queued");
  let repeated = ok(fixture.cli("a", &["delegate", "retry", "queued"]).unwrap()).unwrap();
  let repeated: Value = serde_json::from_slice(&repeated.stdout).unwrap();
  assert_eq!(first["request"], repeated["request"]);
  std::fs::write(&brief, "Different task.\n").unwrap();
  assert!(!fixture.cli("a", &submit).unwrap().status.success());
  for _ in 0..2 {
    let cancelled = ok(fixture.cli("a", &["delegate", "cancel", "queued"]).unwrap()).unwrap();
    assert_eq!(
      serde_json::from_slice::<Value>(&cancelled.stdout).unwrap()["phase"],
      "cancelled"
    );
  }
  setup(&fixture, "a", "beta").unwrap();
  for action in ["status", "cancel"] {
    let receipt = ok(fixture.cli("a", &["delegate", action, "queued"]).unwrap()).unwrap();
    assert_eq!(
      serde_json::from_slice::<Value>(&receipt.stdout).unwrap()["phase"],
      "cancelled"
    );
  }
}

#[test]
fn return_to_retired_endpoint_reuses_pending_generation_after_source_retirement() {
  let fixture = Fixture::new().unwrap();
  pending_return(&fixture, "return-generation").unwrap();
  assert_resumed(&fixture, "return-generation").unwrap();
  setup(&fixture, "a", "alpha").unwrap();
  assert_eq!(
    active(&fixture, "alpha").unwrap()["generation"],
    "return-generation"
  );
}

#[test]
fn pending_enrollment_repairs_missing_destination_marker_after_config_write() {
  let fixture = Fixture::new().unwrap();
  let (_, next) = pending_return(&fixture, "marker-generation").unwrap();
  assert!(!fixture.authority_path("alpha").exists());
  assert!(toolu_state::io::write_atomic(
    &fixture.node_config("alpha"),
    &next.to_string()
  ));
  assert_resumed(&fixture, "marker-generation").unwrap();
}

#[test]
fn lost_enrollment_ack_keeps_later_authoritative_registration() {
  let fixture = Fixture::new().unwrap();
  let (expected, next) = pending_return(&fixture, "ack-generation").unwrap();
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
  register(&fixture, "b", "later-worker").unwrap();
  assert_resumed(&fixture, "ack-generation").unwrap();
  assert!(
    active(&fixture, "alpha").unwrap()["machines"]
      .get("later-worker")
      .is_some()
  );
  let local: Value = serde_json::from_slice(
    &std::fs::read(fixture.controller_root("a").join("delegation/config.json")).unwrap(),
  )
  .unwrap();
  assert!(local["machines"].get("later-worker").is_some());
}

#[test]
fn committed_result_import_is_repeatable_and_requires_matching_origin() {
  let fixture = Fixture::new().unwrap();
  setup(&fixture, "a", "alpha").unwrap();
  let (repo, brief) = fixture.repo().unwrap();
  let submit = [
    "delegate", "submit", "--key", "result", "--repo", &repo, "--brief", &brief, "--agent", "codex",
  ];
  ok(fixture.cli("a", &submit).unwrap()).unwrap();
  ok(fixture.hc(&["kill", "alpha/toolu-relay"], None).unwrap()).unwrap();
  register(&fixture, "a", "worker").unwrap();
  let (attempt, commit) = fixture.seed_result("alpha", "result", &repo).unwrap();
  let partial = format!("{repo}/.git/toolu-transfer/{attempt}/snapshot.bundle");
  std::fs::create_dir_all(std::path::Path::new(&partial).parent().unwrap()).unwrap();
  let bundle = std::fs::read(fixture.controller_root("result.bundle")).unwrap();
  std::fs::write(partial, &bundle[..20]).unwrap();
  std::fs::write(format!("{repo}/uncommitted"), b"keep").unwrap();
  for _ in 0..2 {
    let output = ok(
      fixture
        .cli("a", &["delegate", "results", "result", "--repo", &repo])
        .unwrap(),
    )
    .unwrap();
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["commit"], commit);
    assert_eq!(
      receipt["ref"],
      format!("refs/toolu-transfer/{attempt}/snapshot")
    );
    assert_eq!(
      fixture
        .git(&["-C", &repo, "show", &format!("{commit}:result")])
        .unwrap(),
      "finished"
    );
    assert_eq!(
      std::fs::read(format!("{repo}/uncommitted")).unwrap(),
      b"keep"
    );
  }
  fixture
    .git(&["-C", &repo, "remote", "set-url", "origin", "/different"])
    .unwrap();
  let rejected = fixture
    .cli("a", &["delegate", "results", "result", "--repo", &repo])
    .unwrap();
  assert!(!rejected.status.success());
  assert!(String::from_utf8_lossy(&rejected.stdout).contains("repository origin differs"));
}

#[path = "helpers/delegation_authority_selection.rs"]
mod selection;
