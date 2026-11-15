//! The board header and rows over a real engine: `graph.json` built by the
//! engine's refresh from the GitHub fixture, workers launched into herdr
//! worktrees, and merges driven by the workers' reports.

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use crate::scenario_rig::{Scenario, key};
use crate::server::{Engine, Stop};
use crate::status_api::route;

fn get(engine: &Engine, path: &str) -> Value {
  route(&engine.world, &engine.paths, path, 0, &Secrets::none())
    .expect("route")
    .expect("document")
}

fn header(engine: &Engine) -> Value {
  let doc = get(engine, "/api/v1/epics/e");
  json!([
    doc["waves_total"],
    doc["current_wave"],
    doc["live"],
    doc["queued"]
  ])
}

fn merge(scenario: &Scenario, engine: &mut Engine, number: u64) {
  scenario.close(number);
  scenario.report(engine, number, "ready");
}

#[test]
fn board_header_follows_the_waves_of_a_real_epic_run() {
  let scenario = Scenario::new(&[(2, &[]), (3, &[]), (4, &[2]), (5, &[4])], 2);
  let mut engine = scenario.engine();
  assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  let detail = get(&engine, "/api/v1/epics/e");
  assert_eq!(
    (detail["title"].clone(), detail["url"].clone()),
    (json!("Epic"), json!("u"))
  );
  // Waves 1 (#2, #3), 2 (#4) and 3 (#5): two live in wave 1, two queued.
  assert_eq!(
    header(&engine),
    json!([3, 1, 2, 2]),
    "{:?}",
    scenario.journal()
  );
  let list = get(&engine, "/api/v1/epics");
  let item = &list["items"][0];
  assert_eq!(
    json!([
      item["waves_total"],
      item["current_wave"],
      item["live"],
      item["queued"]
    ]),
    header(&engine)
  );
  for number in [2, 3, 4, 5] {
    let row = detail["issues"]
      .as_array()
      .and_then(|rows| rows.iter().find(|row| row["key"] == key(number).as_str()))
      .expect("row");
    let doc = get(&engine, &format!("/api/v1/issues/{}", key(number)));
    for field in ["model", "effort", "phase_age_ms", "hold"] {
      assert_eq!(row[field], doc[field], "{number} {field}");
    }
  }
  merge(&scenario, &mut engine, 2);
  merge(&scenario, &mut engine, 3);
  assert_eq!(header(&engine), json!([3, 2, 1, 1]));
  merge(&scenario, &mut engine, 4);
  assert_eq!(header(&engine), json!([3, 3, 1, 0]));
  merge(&scenario, &mut engine, 5);
  assert_eq!(header(&engine), json!([3, null, 0, 0]));
}

#[test]
fn graph_issues_carry_their_tracker_url_redacted_or_null() {
  let temp = tempfile::tempdir().expect("temp root");
  let secret = "URL-SECRET-901";
  let kept = crate::support::graph_248().issues[3].url.clone();
  let (paths, world) = super::tests::board(temp.path(), |graph| {
    graph.issues[0].url = format!("https://tracker.example/{secret}/issues/1");
    graph.issues[1].url = String::new();
    graph.issues[2].url = format!("https://t.example/{}", "a".repeat(143));
    graph.issues[4].url = format!("https://t.example/{}", "a".repeat(142));
  });
  let tokens = json!({"peer": secret}).to_string();
  let secrets = crate::fleet_rig::secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let detail = route(&world, &paths, "/api/v1/epics/comemory-248", 0, &secrets)
    .expect("route")
    .expect("document");
  let urls = detail["graph"]["issues"]
    .as_array()
    .expect("graph issues")
    .iter()
    .map(|node| node["url"].clone())
    .collect::<Vec<_>>();
  assert_eq!(
    urls[0],
    json!("https://tracker.example/<redacted>/issues/1")
  );
  // Empty, and one character past the 160 limit: a cut URL is a broken link.
  assert_eq!((&urls[1], &urls[2]), (&Value::Null, &Value::Null));
  assert!(kept.starts_with("https://github.com/"), "{kept}");
  assert_eq!(urls[3], json!(kept));
  assert_eq!(urls[4].as_str().map(|url| url.chars().count()), Some(160));
}
