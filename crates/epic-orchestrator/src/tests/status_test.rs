use super::document;
use crate::logic::ensure_issue;
use crate::model::{Attention, World};
use crate::watch::{Kind, Watch};
use toolu_github::RateLimit;

#[test]
fn an_empty_engine_is_down() {
  let world = World::new(0);
  let doc = document(&world, false, None);
  assert_eq!(doc["engine"], "down");
  assert_eq!(doc["paused"], false);
  assert_eq!(doc["epics"], serde_json::json!([]));
  assert_eq!(doc["issues"], serde_json::json!([]));
  assert_eq!(doc["attention"], serde_json::json!([]));
}

#[test]
fn one_epic_keeps_its_issues_and_open_attention() {
  let mut world = World::new(1);
  world.paused.insert("one".to_owned());
  {
    let issue = ensure_issue(&mut world, "a", "one", "/epic");
    issue.phase = "execution".to_owned();
    issue.stage = "running".to_owned();
  }
  ensure_issue(&mut world, "b", "falconiere-toolu-402", "/other");
  world.attention.push(item("a", "one", false));
  world
    .attention
    .push(item("b", "falconiere-toolu-402", true));
  let doc = document(&world, true, Some("one"));
  assert_eq!(doc["engine"], "running");
  assert_eq!(doc["paused"], true);
  assert_eq!(doc["issues"].as_array().expect("issues").len(), 1);
  assert_eq!(doc["epics"].as_array().expect("epics").len(), 1);
  assert_eq!(doc["attention"].as_array().expect("attention").len(), 1);
  let suffix = document(&world, true, Some("402"));
  assert_eq!(suffix["issues"][0]["key"], "b");
}

#[test]
fn status_shows_separate_github_budgets_and_pr_deadlines() {
  let mut world = World::new(180_000);
  let issue = ensure_issue(&mut world, "one", "epic", "/epic");
  issue.phase = "babysit".into();
  issue.pr = Some(1);
  let mut watch = Watch::new(
    Kind::Pr {
      key: "one".into(),
      repo: "o/r".into(),
      number: 1,
    },
    0,
  );
  watch.last_at_ms = Some(180_000);
  watch.next_at_ms = 360_000;
  world.watches.insert("pr:o/r#1".into(), watch);
  world.rest_rate = Some(RateLimit {
    remaining: Some(80),
    reset: Some(500),
    ..RateLimit::default()
  });
  world.rest_points = 4;
  world.graphql_remaining = Some(19);
  world.graphql_reset_at = Some(500);
  world.graphql_points = 2;
  let doc = document(&world, true, None);
  assert_eq!(doc["github"]["rest"]["points"], 4);
  assert_eq!(doc["github"]["rest"]["rate"]["remaining"], 80);
  assert_eq!(doc["github"]["graphql"]["points"], 2);
  assert_eq!(doc["github"]["graphql"]["remaining"], 19);
  assert_eq!(doc["github"]["effectsHeld"], true);
  assert_eq!(doc["issues"][0]["github"]["lastCheckAt"], 180_000);
  assert_eq!(doc["issues"][0]["github"]["nextCheckAt"], 360_000);
}

fn item(key: &str, epic: &str, delivered: bool) -> Attention {
  Attention {
    seq: 1,
    kind: "stall".to_owned(),
    key: key.to_owned(),
    epic: epic.to_owned(),
    note: "n".to_owned(),
    delivered,
  }
}
