use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use super::{limit_line as scan_line, on_status};
use crate::model::Report;
use crate::paths::Paths;
use crate::server::{Engine, Fault};

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

#[test]
fn blocked_pending_reaches_wait_only_and_raises_once_per_spell() {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  crate::disk::write_value(
    &epic.join("issues").join("a.json"),
    &json!({"title": "A", "url": "https://github.com/o/r/issues/1"}),
  )
  .expect("issue");
  let paths = Paths::at(tmp.path().join("resources"));
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
  let wait = crate::status::wait_body(&mut engine.world, 0);
  assert_eq!(wait["kind"], "blocked");
  assert_eq!(
    wait["pending"],
    "cargo build\n   Compiling a\nDo you want to run rm -rf build? (y/n)"
  );
}
