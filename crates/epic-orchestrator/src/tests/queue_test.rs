use serde_json::json;

use super::{document, head, queued, queues};
use crate::epics::Epic;
use crate::model::{Issue, World};

/// Epic #248's real graph, with ready issues `(key, ready at ms)`.
fn world(ready: &[(&str, u64)]) -> World {
  let mut world = World::new(1_000);
  let graph = crate::support::graph_248();
  let mut epic = Epic::new("e", "/state", 0, Some("github"));
  for (key, at) in ready {
    let node = graph.find(key).expect("graph issue");
    let mut issue = Issue::blank(key, "e", "/state", 0);
    "ready".clone_into(&mut issue.phase);
    "running".clone_into(&mut issue.stage);
    node.repo.clone_into(&mut issue.repo);
    issue.pr = node.number;
    issue.queue.queued_at_ms = Some(*at);
    world.issues.insert((*key).to_owned(), issue);
  }
  epic.graph = Some(graph);
  world.epics.insert("e".to_owned(), epic);
  world
}

fn keys(world: &World, repo: &str) -> Vec<String> {
  queues(world)
    .get(repo)
    .map(|entries| entries.iter().map(|entry| entry.key.clone()).collect())
    .unwrap_or_default()
}

const READY: [(&str, u64); 5] = [
  ("comemory-256", 10),
  ("comemory-251", 30),
  ("comemory-253", 20),
  ("comemory-252", 20),
  ("comemory-io-184", 5),
];

#[test]
fn each_repository_orders_by_unblocks_then_ready_time_then_key() {
  let world = world(&READY);
  assert_eq!(
    keys(&world, "Falconiere/comemory"),
    [
      "comemory-252",
      "comemory-253",
      "comemory-251",
      "comemory-256"
    ]
  );
  assert_eq!(keys(&world, "CodaSignal/comemory.io"), ["comemory-io-184"]);
  assert_eq!(
    head(&world, "Falconiere/comemory").as_deref(),
    Some("comemory-252")
  );
}

#[test]
fn only_ready_running_issues_with_a_pull_request_are_queued() {
  let mut world = world(&READY);
  let issue = |world: &mut World, key: &str| world.issues.get_mut(key).expect("issue").clone();
  for (key, change) in [
    ("comemory-252", "babysit"),
    ("comemory-253", "merged"),
    ("comemory-251", "no-pr"),
  ] {
    let mut edited = issue(&mut world, key);
    match change {
      "babysit" => "babysit".clone_into(&mut edited.phase),
      "merged" => "cleaning".clone_into(&mut edited.stage),
      _ => edited.pr = None,
    }
    assert!(!queued(&edited), "{key} {change}");
    world.issues.insert(key.to_owned(), edited);
  }
  assert_eq!(keys(&world, "Falconiere/comemory"), ["comemory-256"]);
  let mut waiting = issue(&mut world, "comemory-256");
  "awaiting_merge".clone_into(&mut waiting.stage);
  waiting.queue.queued_at_ms = None;
  assert!(queued(&waiting), "awaiting_merge stays queued");
}

#[test]
fn a_paused_epic_is_listed_but_never_the_head() {
  let mut world = world(&READY);
  world.paused.insert("e".to_owned());
  assert_eq!(head(&world, "Falconiere/comemory"), None);
  assert_eq!(keys(&world, "Falconiere/comemory").len(), 4);
  world.paused.clear();
  world.paused_all = true;
  assert_eq!(head(&world, "CodaSignal/comemory.io"), None);
}

#[test]
fn the_document_numbers_positions_marks_the_head_and_filters_by_epic() {
  let mut world = world(&READY);
  world
    .issues
    .get_mut("comemory-252")
    .expect("issue")
    .queue
    .rebase_rounds
    .engine = 1;
  let doc = document(&world, None);
  let repo = &doc["repositories"][1];
  assert_eq!(repo["repo"], "Falconiere/comemory");
  assert_eq!(
    repo["items"][0],
    json!({"position": 1, "key": "comemory-252", "epic": "e", "pr": 252, "unblocks": 2,
      "queued_at": "1970-01-01T00:00:00.020Z", "head": true,
      "rebase_rounds": {"engine": 1, "worker": 0}})
  );
  assert_eq!(repo["items"][3]["position"], 4);
  assert_eq!(repo["items"][3]["head"], false);
  assert_eq!(
    document(&world, Some("other")),
    json!({"schema": 1, "repositories": []})
  );
}

#[test]
fn the_order_survives_a_reload_of_the_records() {
  let tmp = tempfile::tempdir().expect("temp");
  let state = tmp.path().display().to_string();
  std::fs::create_dir_all(tmp.path().join("issues")).expect("issues");
  let mut world = world(&READY);
  for issue in world.issues.values_mut() {
    state.clone_into(&mut issue.state_dir);
    crate::disk::write_issue(issue, std::time::SystemTime::now()).expect("write");
  }
  let before = keys(&world, "Falconiere/comemory");
  for issue in world.issues.values_mut() {
    issue.queue.queued_at_ms = None;
    let record = crate::disk::read_value(
      &tmp
        .path()
        .join("issues")
        .join(format!("{}.json", issue.key)),
    )
    .expect("record");
    crate::queue_fields::read(issue, &record);
  }
  assert_eq!(keys(&world, "Falconiere/comemory"), before);
}

#[test]
fn only_a_running_or_awaiting_merge_head_is_armed() {
  let mut world = world(&READY);
  let head = "comemory-252";
  "awaiting_merge".clone_into(&mut world.issues.get_mut(head).expect("head").stage);
  crate::logic::arm_merge(&mut world, head);
  assert!(
    world.issues[head].pending.is_some(),
    "an awaiting_merge head is armed"
  );
  for stage in ["cleaning", "merged", "closed"] {
    let mut world = self::world(&READY);
    stage.clone_into(&mut world.issues.get_mut(head).expect("head").stage);
    crate::logic::arm_merge(&mut world, head);
    assert!(world.issues[head].pending.is_none(), "{stage}");
  }
  let mut world = self::world(&READY);
  crate::logic::arm_merge(&mut world, "comemory-256");
  assert!(
    world.issues["comemory-256"].pending.is_none(),
    "not the head"
  );
}
