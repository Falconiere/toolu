use super::*;
use crate::fleet_rig::secrets_of;
use crate::herdr_fixture::HerdrFixture;
use crate::herdr_state::Scripted;
use crate::model::Issue;

#[test]
fn worker_screen_cache_churn_bounds_entries_and_screen_bytes() {
  let fixture = HerdrFixture::start();
  fixture.state().tail = format!("{}TAIL-Ω", "x".repeat(70_000));
  let mut world = World::new(0);
  for index in 0..65 {
    let issue = Issue::blank(&format!("worker{index}"), "epic", "/tmp/epic", 0);
    world.issues.insert(issue.key.clone(), issue);
  }
  let world = Mutex::new(world);
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let limiter = Limiter::default();
  for index in 0..65 {
    let response = screen.response(
      &format!("worker{index}"),
      &limiter,
      &world,
      &Secrets::none(),
    );
    assert_eq!(response.status, 200);
  }
  let cached = screen.cached.lock().expect("cache");
  let bytes: usize = cached
    .values()
    .map(|cell| {
      cell
        .lock()
        .expect("sample")
        .as_ref()
        .and_then(|sample| sample.result.as_ref().ok())
        .map_or(0, String::len)
    })
    .sum();
  println!(
    "platform={}-{} valid_keys=65 raw_screen_bytes=70007 retained_entries={} retained_screen_bytes={} herdr_reads={}",
    std::env::consts::OS,
    std::env::consts::ARCH,
    cached.len(),
    bytes,
    fixture.calls().len()
  );
  assert!(cached.len() <= 64);
  assert!(bytes <= 64 * 65_536);
  assert!(!cached.contains_key("worker0"));
  assert!(cached.contains_key("worker64"));
}

#[test]
fn worker_screen_secret_rotation_redacts_before_tail_cut() {
  let fixture = HerdrFixture::start();
  let root = tempfile::tempdir().expect("config root");
  let token = "Z".repeat(300);
  let before = 70_000 - 65_536 - 5;
  fixture.state().tail = format!(
    "{}{}{}",
    "x".repeat(before),
    token,
    "y".repeat(70_000 - before - token.len())
  );
  let first_secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", &token)]);
  let world = Mutex::new(super::tests::world_with("worker", "working"));
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let limiter = Limiter::default();
  let first = screen.response("worker", &limiter, &world, &first_secrets);
  let first: Value = serde_json::from_slice(&first.body).expect("first screen");
  let text = first["text"].as_str().expect("screen text");
  assert!(text.contains("<redacted>"));
  assert!(!text.contains(&"Z".repeat(50)), "secret fragment survived");

  fixture.state().tail = "ROTATED-SECRET NEW".to_owned();
  let rotated = secrets_of(
    root.path(),
    &[("TOOLU_EPIC_STATUS_TOKEN", "ROTATED-SECRET")],
  );
  let second = screen.response("worker", &limiter, &world, &rotated);
  let second: Value = serde_json::from_slice(&second.body).expect("rotated screen");
  assert_eq!(fixture.calls().len(), 2, "rotation must resample");
  assert_eq!(second["text"], "<redacted> NEW");
}

#[test]
fn worker_screen_error_redacts_before_retention_cap() {
  let fixture = HerdrFixture::start();
  let token = "Z".repeat(300);
  fixture.script(
    "agent.read",
    Scripted::Error {
      code: token.clone(),
      message: "unavailable".to_owned(),
    },
  );
  let root = tempfile::tempdir().expect("config root");
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", &token)]);
  let world = Mutex::new(super::tests::world_with("worker", "working"));
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let response = screen.response("worker", &Limiter::default(), &world, &secrets);
  let body: Value = serde_json::from_slice(&response.body).expect("error body");
  assert_eq!(response.status, 200);
  assert_eq!(body["reason"], "error: <redacted>");
}

#[test]
fn worker_screen_redaction_marker_is_not_redacted_twice() {
  let fixture = HerdrFixture::start();
  fixture.state().tail = "redacted".to_owned();
  let root = tempfile::tempdir().expect("config root");
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", "redacted")]);
  let world = Mutex::new(super::tests::world_with("worker", "working"));
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let response = screen.response("worker", &Limiter::default(), &world, &secrets);
  let body: Value = serde_json::from_slice(&response.body).expect("screen body");
  assert_eq!(body["text"], "<redacted>");
}

#[test]
fn worker_screen_owned_cells_survive_churn_and_capacity_pressure() {
  let fixture = HerdrFixture::start();
  let mut world = World::new(0);
  for index in 0..=64 {
    let issue = Issue::blank(&format!("worker{index}"), "epic", "/tmp/epic", 0);
    world.issues.insert(issue.key.clone(), issue);
  }
  let world = Mutex::new(world);
  let screen = Screen::new(Herdr::Socket(fixture.socket.clone()), None);
  let limiter = Limiter::default();
  let mut owned = (0..64)
    .map(|index| {
      screen
        .cell(&format!("worker{index}"))
        .unwrap_or_else(|response| panic!("cell status {}", response.status))
    })
    .collect::<Vec<_>>();
  let busy = screen.response("worker64", &limiter, &world, &Secrets::none());
  assert_eq!(busy.status, 503);
  assert!(fixture.calls().is_empty(), "full cache sampled a new cell");

  let hot = owned.remove(0);
  drop(owned);
  let sampled = screen.response("worker64", &limiter, &world, &Secrets::none());
  assert_eq!(sampled.status, 200);
  let cached = screen.cached.lock().expect("cache");
  assert_eq!(cached.len(), 64);
  assert!(Arc::ptr_eq(&hot, cached.get("worker0").expect("hot cell")));
  assert_eq!(fixture.calls().len(), 1);
}
