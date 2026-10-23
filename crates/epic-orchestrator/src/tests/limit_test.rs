use std::time::SystemTime;

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use super::{limit_line as scan_line, on_status};
use crate::journal::{self, Record};
use crate::model::{Ack, Report, Step, World};
use crate::paths::Paths;
use crate::server::{Engine, Fault};

const MINUTE: u64 = 60 * 1000;

#[test]
fn limit_line() {
  assert!(scan_line("All 34 tests passed").is_none());
  assert_eq!(
    scan_line("ok\nusage limit reached\n").as_deref(),
    Some("usage limit reached")
  );
  assert_eq!(
    scan_line("usage limit first\nquota exceeded\n").as_deref(),
    Some("quota exceeded")
  );
  assert!(scan_line("You've hit your usage limit. Upgrade or try again at 5:00 PM.").is_some());
  assert!(scan_line("Error: 429 Too Many Requests").is_some());
  assert!(scan_line("stream error: rate limited, retrying").is_some());
  let long = format!("{} usage limit", "x".repeat(210));
  let got = scan_line(&long).expect("long");
  assert_eq!(got.chars().count(), 200);
}

fn running_issue(root: &std::path::Path) -> (Engine, Paths) {
  let epic = root.join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  crate::disk::write_value(
    &epic.join("issues").join("a.json"),
    &json!({"title": "A", "url": "https://github.com/o/r/issues/1"}),
  )
  .expect("issue");
  let paths = Paths::at(root.join("resources"));
  let mut engine = Engine::open(paths.clone(), None, Fault::None).expect("open");
  engine
    .report(&Report {
      key: "a".to_owned(),
      epic: "one".to_owned(),
      state_dir: epic.display().to_string(),
      phase: "execution".to_owned(),
      pr: None,
      note: "working".to_owned(),
    })
    .expect("report");
  (engine, paths)
}

#[test]
fn blocked_pending_reaches_wait_only_and_raises_once_per_spell() {
  let tmp = tempfile::tempdir().expect("temp");
  let (mut engine, paths) = running_issue(tmp.path());
  let tail = "cargo build\n\n   Compiling a\nDo you want to run rm -rf build? (y/n)   \n";
  on_status(&mut engine, "a", "blocked", tail).expect("blocked");
  on_status(&mut engine, "a", "blocked", tail).expect("still blocked");
  assert_eq!(engine.world.attention.len(), 1);
  for path in ["/api/v1/attention", "/api/v1/overview", "/api/v1/issues/a"] {
    let body = crate::status_api::route(&engine.world, &paths, path, 0, &Secrets::none())
      .expect("route")
      .expect("known path")
      .to_string();
    assert!(!body.contains("rm -rf build"), "{path}: {body}");
  }
  let wait = crate::status::wait_body(&mut engine.world, None).0;
  assert_eq!(wait["kind"], "blocked");
  assert_eq!(
    wait["pending"],
    "cargo build\n   Compiling a\nDo you want to run rm -rf build? (y/n)"
  );
  assert_eq!(
    crate::status::wait_body(&mut engine.world, None).0["state"],
    "waiting"
  );
}

#[test]
fn blocked_pending_of_a_new_spell_reopens_the_seen_item() {
  let tmp = tempfile::tempdir().expect("temp");
  let (mut engine, _paths) = running_issue(tmp.path());
  on_status(&mut engine, "a", "blocked", "Run tests? (y/n)").expect("blocked");
  let first = crate::status::wait_body(&mut engine.world, None).0;
  crate::attention::ack(&mut engine.world, "a", "user");
  on_status(&mut engine, "a", "working", "").expect("working");
  on_status(&mut engine, "a", "blocked", "Overwrite config? (y/n)").expect("blocked again");
  assert_eq!(engine.world.attention.len(), 1);
  let second = crate::status::wait_body(&mut engine.world, None).0;
  assert_eq!(
    (&second["state"], &second["seq"]),
    (&json!("open"), &first["seq"])
  );
  assert_eq!(second["pending"], "Overwrite config? (y/n)");
}

#[test]
fn reraise_reopens_a_seen_item_and_the_rebuild_replays_it() {
  let mut world = World::new(0);
  let token = crate::attention::reraise(&mut world, "stall", "a", "stalled");
  world.outbox.clear();
  assert_eq!(
    crate::attention::reraise(&mut world, "stall", "a", "stalled"),
    token
  );
  assert_eq!(world.outbox.len(), 0);
  crate::attention::ack(&mut world, "a", "user");
  world.now_ms = 5 * MINUTE;
  assert_eq!(
    crate::attention::reraise(&mut world, "stall", "a", "stalled again"),
    token
  );
  let item = &world.attention[0];
  assert_eq!(
    (item.state, item.delivered, item.announced_ms),
    (Ack::Open, false, 5 * MINUTE)
  );
  assert_eq!(item.note, "stalled again");
  assert_eq!(
    world
      .outbox
      .iter()
      .filter(|step| matches!(step, Step::Journal(row) if row.name == "reopened"))
      .count(),
    1
  );
  assert_eq!(
    world
      .outbox
      .iter()
      .filter(|step| matches!(
        step,
        Step::Notify {
          reminder: false,
          ..
        }
      ))
      .count(),
    1
  );
  assert_eq!(
    crate::attention::next_reminder_ms(&world),
    Some(35 * MINUTE)
  );
  assert_reopen_replays(&token);
}

fn assert_reopen_replays(token: &str) {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  let now = SystemTime::now();
  for row in [
    Record::new("judgment", "stall", "a", token, "stalled"),
    Record::new("judgment", "acknowledged", "a", token, "by=user"),
    Record::new("judgment", "reopened", "a", token, "stalled again"),
  ] {
    journal::append(&paths.journal_dir(), &paths.journal_lock(), &row, now).expect("append");
  }
  let rebuilt = crate::snapshot::load(&paths, now).expect("load");
  let item = &rebuilt.attention[0];
  assert_eq!(
    (item.state, item.note.as_str()),
    (Ack::Open, "stalled again")
  );
}
