use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use super::list;
use crate::disk::write_value;
use crate::model::{Action, Pending, World};
use crate::paths::Paths;
use crate::status_api::route;

const EPIC: &str = "comemory-248";

#[test]
fn empty_registered_epic_still_has_a_status_board() {
  let temp = tempfile::tempdir().expect("temp root");
  let paths = Paths::at(temp.path());
  write_value(
    &paths.registry(),
    &json!({
      "version": 1,
      "epics": [{"key": "repo-402", "state_dir": temp.path().join("epic")}]
    }),
  )
  .expect("registry");
  let items = list(&World::new(0), &paths, &Secrets::none()).expect("epic list");
  assert_eq!(items.len(), 1);
  assert_eq!(items[0]["key"], "repo-402");
  assert_eq!(items[0]["counts"], json!({}));
  assert_eq!(items[0]["title"], Value::Null);
  assert_eq!(items[0]["url"], Value::Null);
  assert_eq!(items[0]["waves_total"], 0);
  assert_eq!(items[0]["current_wave"], Value::Null);
  assert_eq!(
    (items[0]["live"].clone(), items[0]["queued"].clone()),
    (json!(0), json!(0))
  );
}

/// The epic #248 graph, changed by `edit`, saved under `root` and loaded by
/// the engine's snapshot.
fn board(root: &std::path::Path, edit: impl FnOnce(&mut crate::graph::Graph)) -> (Paths, World) {
  let paths = Paths::at(root.join("resources"));
  let state = root.join("state");
  let mut graph = crate::support::graph_248();
  graph.state_dir = state.display().to_string();
  edit(&mut graph);
  crate::graph::write_graph(&state.join("graph.json"), &graph).expect("graph");
  write_value(
    &paths.registry(),
    &json!({"version": 1, "epics": [{"key": EPIC, "state_dir": state}]}),
  )
  .expect("registry");
  let world = crate::snapshot::load(&paths, SystemTime::now()).expect("world");
  (paths, world)
}

fn set_stage(world: &mut World, key: &str, stage: &str) {
  world.issues.get_mut(key).expect("issue").stage = stage.to_owned();
}

fn get(world: &World, paths: &Paths, path: &str, secrets: &Secrets) -> Value {
  route(world, paths, path, 0, secrets)
    .expect("route")
    .expect("document")
}

fn header(doc: &Value) -> Value {
  json!([
    doc["waves_total"],
    doc["current_wave"],
    doc["live"],
    doc["queued"]
  ])
}

#[test]
fn header_counts_waves_live_and_queued_from_the_engine_graph() {
  let temp = tempfile::tempdir().expect("temp root");
  let (paths, mut world) = board(temp.path(), |_| {});
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(
    detail["title"],
    "[Realtime replication] Shared data everywhere with a required resident daemon"
  );
  assert_eq!(
    detail["url"],
    "https://github.com/Falconiere/comemory/issues/248"
  );
  // Over every issue the waves are #249; #250; #251-#255 and io#183; #256,
  // #257, io#184; #258, io#185; tap#1. The five closed before the run fill
  // waves 1-2 and part of 3, so open work starts in wave 3; nine are queued.
  assert_eq!(header(&detail), json!([6, 3, 0, 9]));
  set_stage(&mut world, "comemory-254", "running");
  set_stage(&mut world, "comemory-255", "running");
  set_stage(&mut world, "comemory-io-183", "awaiting_merge");
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(header(&detail), json!([6, 3, 3, 6]));
  let items = get(&world, &paths, "/api/v1/epics", &Secrets::none());
  assert_eq!(header(&items["items"][0]), header(&detail));
  assert_eq!(items["items"][0]["title"], detail["title"]);
}

#[test]
fn current_wave_moves_past_merged_and_abandoned_issues_then_is_null() {
  let temp = tempfile::tempdir().expect("temp root");
  let (paths, mut world) = board(temp.path(), |_| {});
  // Merged pull requests still cleaning up count as done.
  set_stage(&mut world, "comemory-254", "cleaning");
  set_stage(&mut world, "comemory-255", "abandoned");
  set_stage(&mut world, "comemory-io-183", "cleanup-incomplete");
  set_stage(&mut world, "comemory-256", "running");
  // A launch in flight is neither queued nor live yet.
  world.issues.get_mut("comemory-257").expect("257").pending = Some(Pending {
    action: Action::Launch,
    token: "t".to_owned(),
    phase: 0,
    recovered: false,
    follow: None,
    reroute: false,
  });
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(header(&detail), json!([6, 4, 1, 4]));
  // The graph block keeps the same stable numbering as the header.
  assert_eq!(
    detail["graph"]["waves"][0],
    json!({"wave": 1, "issues": ["comemory-249"]})
  );
  assert_eq!(detail["graph"]["waves"].as_array().map(Vec::len), Some(6));
  let keys: Vec<String> = world.issues.keys().cloned().collect();
  for key in keys {
    set_stage(&mut world, &key, "merged");
  }
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(header(&detail), json!([6, null, 0, 0]));
}

#[test]
fn a_stale_cycle_between_closed_issues_keeps_the_open_waves() {
  let temp = tempfile::tempdir().expect("temp root");
  // #250 is blocked by #249; this closes a cycle only the closed issues see.
  let (paths, world) = board(temp.path(), |graph| {
    if let Some(closed) = graph
      .issues
      .iter_mut()
      .find(|row| row.key == "comemory-249")
    {
      closed
        .blockers
        .insert("Falconiere/comemory#250".into(), "closed".into());
    }
  });
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(header(&detail), json!([4, 1, 0, 9]));
}

#[test]
fn a_long_title_is_cut_and_a_secret_in_it_redacted() {
  let temp = tempfile::tempdir().expect("temp root");
  let secret = "TITLE-SECRET-657";
  let title = format!("ship {secret} {}", "x".repeat(200));
  let (paths, world) = board(temp.path(), |graph| {
    title.clone_into(&mut graph.epic.title);
  });
  let tokens = json!({"peer": secret}).to_string();
  let secrets = crate::fleet_rig::secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let expected: String = title
    .replace(secret, "<redacted>")
    .chars()
    .take(160)
    .collect();
  for path in [
    format!("/api/v1/epics/{EPIC}"),
    "/api/v1/overview".to_owned(),
  ] {
    let doc = get(&world, &paths, &path, &secrets);
    assert!(!doc.to_string().contains(secret), "{path}");
    let served = if path.ends_with("overview") {
      &doc["epics"][0]["title"]
    } else {
      &doc["title"]
    };
    assert_eq!(served, &json!(expected), "{path}");
  }
  let exact = "y".repeat(160);
  let (paths, world) = board(&temp.path().join("exact"), |graph| {
    exact.clone_into(&mut graph.epic.title);
  });
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert_eq!(detail["title"], json!(exact));
}

#[test]
fn a_secret_across_the_cut_never_leaks_and_a_long_url_is_left_out() {
  let temp = tempfile::tempdir().expect("temp root");
  let secret = "STRADDLE-SECRET-657";
  let title = format!("{}{secret} tail", "x".repeat(150));
  let url = format!("https://jira.example/browse/{}", "a".repeat(140));
  let (paths, world) = board(temp.path(), |graph| {
    title.clone_into(&mut graph.epic.title);
    url.clone_into(&mut graph.epic.url);
  });
  let tokens = json!({"peer": secret}).to_string();
  let secrets = crate::fleet_rig::secrets_of(temp.path(), &[("TOOLU_EPIC_PEER_TOKENS", &tokens)]);
  let detail = get(&world, &paths, &format!("/api/v1/epics/{EPIC}"), &secrets);
  let served = detail["title"].as_str().expect("title");
  assert!(!served.contains("STRADDLE"), "{served}");
  assert_eq!(served, format!("{}<redacted>", "x".repeat(150)));
  assert_eq!(detail["url"], Value::Null);
}

/// The board with a route for comemory-254 and a held launch record for
/// comemory-255; comemory-256 has neither.
fn held_board(root: &std::path::Path) -> (Paths, World) {
  let state = root.join("state");
  write_value(
    &state.join("routes/comemory-254.json"),
    &json!({"host": "claude", "model": "opus", "effort": "high"}),
  )
  .expect("route");
  let hold = json!({"kind": "coupling", "with": "comemory-254", "score": 0.91,
    "source": "PRIVATE-HOLD-SOURCE", "at": "2026-10-27T12:00:00Z"});
  write_value(
    &state.join("issues/comemory-255.json"),
    &json!({"stage": "running", "kind": "codex", "model": "gpt-6-sol", "hold": hold}),
  )
  .expect("record");
  let (paths, mut world) = board(root, |_| {});
  set_stage(&mut world, "comemory-254", "running");
  world.now_ms = world.now_ms.saturating_add(1_500);
  (paths, world)
}

fn worker(doc: &Value) -> Value {
  json!([
    doc["model"],
    doc["effort"],
    doc["phase_age_ms"],
    doc["hold"]
  ])
}

#[test]
fn rows_carry_the_worker_fields_of_the_issue_document_and_a_private_hold() {
  let temp = tempfile::tempdir().expect("temp root");
  let (paths, world) = held_board(temp.path());
  let detail = get(
    &world,
    &paths,
    &format!("/api/v1/epics/{EPIC}"),
    &Secrets::none(),
  );
  assert!(!detail.to_string().contains("PRIVATE-HOLD-SOURCE"));
  let rows = detail["issues"].as_array().expect("rows");
  let row = |key: &str| rows.iter().find(|row| row["key"] == key).expect("row");
  for key in ["comemory-254", "comemory-255", "comemory-256"] {
    let doc = get(
      &world,
      &paths,
      &format!("/api/v1/issues/{key}"),
      &Secrets::none(),
    );
    assert_eq!(worker(row(key)), worker(&doc), "{key}");
  }
  assert_eq!(
    worker(row("comemory-254")),
    json!(["opus", "high", 1_500, null])
  );
  let hold = json!({"kind": "coupling", "with": "comemory-254", "at": "2026-10-27T12:00:00Z"});
  assert_eq!(
    worker(row("comemory-255")),
    json!(["gpt-6-sol", null, 1_500, hold])
  );
  assert_eq!(
    worker(row("comemory-256")),
    json!([null, null, 1_500, null])
  );
}
