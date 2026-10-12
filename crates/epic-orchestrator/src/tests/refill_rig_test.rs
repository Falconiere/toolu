//! Worlds for refill and hold tests: one epic `e` whose graph gives each
//! issue its chain and unblocked count.

use serde_json::json;

use crate::epics::Epic;
use crate::graph::Graph;
use crate::model::{Issue, World};

/// `(key, chain, unblocks)` issues of epic `e`, all ready, at `max` slots.
pub(crate) fn world(issues: &[(&str, u64, u64)], max: u32) -> World {
  let mut world = World::new(1_000_000);
  let rows: Vec<serde_json::Value> = issues
    .iter()
    .enumerate()
    .map(|(index, (key, chain, unblocks))| {
      json!({"ref": format!("o/r#{}", index + 1), "title": key, "state": "open", "url": "", "repo": "o/r",
        "number": index + 1, "key": key, "status": "ready", "chain": chain, "unblocks": unblocks})
    })
    .collect();
  let graph: Graph = serde_json::from_value(json!({
    "epic": {"ref": "o/r#100", "title": "E", "state": "open", "url": ""},
    "state_dir": "/state", "max_parallel": max, "issues": rows,
  }))
  .expect("graph");
  for (key, _, _) in issues {
    world
      .issues
      .insert((*key).to_owned(), Issue::blank(key, "e", "/state", 0));
  }
  let mut epic = Epic::new("e", "/state", 0, Some("github"));
  epic.reference = "o/r#100".into();
  epic.max_parallel = max;
  epic.graph = Some(graph);
  world.epics.insert("e".into(), epic);
  world
}

/// Keys with a pending launch.
pub(crate) fn armed(world: &World) -> Vec<String> {
  world
    .issues
    .values()
    .filter(|issue| {
      issue
        .pending
        .as_ref()
        .is_some_and(|pending| pending.action == crate::model::Action::Launch)
    })
    .map(|issue| issue.key.clone())
    .collect()
}

/// The batch keys of epic `e`.
pub(crate) fn batch(world: &World) -> Vec<String> {
  world.epics["e"]
    .batch
    .iter()
    .map(|member| member.key.clone())
    .collect()
}

/// Journal names and notes queued in the outbox.
pub(crate) fn journaled(world: &World) -> Vec<(String, String)> {
  world
    .outbox
    .iter()
    .filter_map(|step| match step {
      crate::model::Step::Journal(record) => Some((record.name.clone(), record.note.clone())),
      crate::model::Step::Call { .. }
      | crate::model::Step::Status { .. }
      | crate::model::Step::Issue { .. }
      | crate::model::Step::Prompt { .. }
      | crate::model::Step::Notify { .. } => None,
    })
    .collect()
}
