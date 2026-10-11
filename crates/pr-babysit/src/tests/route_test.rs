use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_runtime::env::Env;

use super::jev::Scoring;
use super::{Request, on_path, route};
use crate::fixer::config::{Settings, parse};
use crate::fixer::host::Fixer;
use crate::fixture;

const NOW: u64 = 1_790_596_800;
const RAISED: &str = "PRRT_kwDOSzUwAc6d2Ypf";

struct World {
  dir: TempDir,
  env: Env,
}

fn world() -> World {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir(&bin).unwrap();
  for cli in ["codex", "claude"] {
    std::fs::write(bin.join(cli), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join(cli), std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  std::fs::write(bin.join("cursor-agent"), "not executable").unwrap();
  let env = Env::from_pairs([("PATH", format!("/nonexistent::{}", bin.display()))]);
  World { dir, env }
}

fn settings(block: &Value) -> Settings {
  parse(block.as_object().unwrap(), Fixer::Claude).unwrap()
}

fn golden(name: &str) -> Value {
  fixture::json("route/route-plans.json")[name].clone()
}

fn plan(
  world: &World,
  settings: &Settings,
  raise: &[String],
  state: Option<&Path>,
  scoring: Scoring<'_>,
) -> Value {
  let items = fixture::path("items/review-items.json");
  let request = Request {
    items: &items,
    state_file: state,
    raise,
    now: NOW,
  };
  route(&request, settings, &world.env, scoring).unwrap()
}

fn answers() -> PathBuf {
  fixture::path("jev/fix-tiers.json")
}

#[test]
fn inline_plans_match_the_typescript_router() {
  let world = world();
  let inline = settings(&json!({"dispatch": "inline"}));
  assert_eq!(
    plan(&world, &inline, &[], None, Scoring::Off),
    golden("heuristicInline")
  );
  let answers = answers();
  assert_eq!(
    plan(&world, &inline, &[], None, Scoring::Replay(&answers)),
    golden("jevInline")
  );
  let raise = [RAISED.to_owned()];
  assert_eq!(
    plan(&world, &inline, &raise, None, Scoring::Replay(&answers)),
    golden("raised")
  );
}

#[test]
fn herdr_plans_skip_cooling_hosts_and_hosts_off_path() {
  let world = world();
  let block = json!({"hosts": ["cursor", "codex", "claude"], "prefer": {"critical": ["claude"]}, "unattended": false});
  let herdr = settings(&block);
  let state = world.dir.path().join("state.json");
  let cooldowns = json!({"hostCooldowns": {
    "claude": {"until": "2026-09-28T13:00:00Z"},
    "opencode": {"until": "2026-09-28T11:00:00Z"}
  }});
  std::fs::write(&state, cooldowns.to_string()).unwrap();
  assert_eq!(
    plan(&world, &herdr, &[], Some(&state), Scoring::Off),
    golden("herdr")
  );
  let answers = answers();
  assert_eq!(
    plan(&world, &herdr, &[], None, Scoring::Replay(&answers)),
    golden("herdrJev")
  );
}

#[test]
fn every_host_cooling_routes_inline_and_says_why() {
  let world = world();
  let both = settings(&json!({"hosts": ["codex", "claude"]}));
  let state = world.dir.path().join("state.json");
  let until = json!({"until": "2026-09-28T13:00:00Z"});
  std::fs::write(
    &state,
    json!({"hostCooldowns": {"codex": until, "claude": until}}).to_string(),
  )
  .unwrap();
  let routed = plan(&world, &both, &[], Some(&state), Scoring::Off);
  assert_eq!(routed["dispatch"], "inline");
  let note = routed["note"].as_str().unwrap();
  assert!(
    note.ends_with("no fixer host available (cooling: claude, codex)"),
    "{note}"
  );
  assert!(
    routed["groups"]
      .as_array()
      .unwrap()
      .iter()
      .all(|group| group["host"].is_null())
  );
  let cursor = settings(&json!({"hosts": ["cursor"]}));
  let missing = plan(&world, &cursor, &[], None, Scoring::Off);
  assert!(
    missing["note"]
      .as_str()
      .unwrap()
      .contains("(CLI not on PATH: cursor)")
  );
}

#[test]
fn a_missing_items_file_or_a_bad_raise_is_a_plan_error() {
  let world = world();
  let request = Request {
    items: &world.dir.path().join("absent.json"),
    state_file: None,
    raise: &[],
    now: NOW,
  };
  let failure = route(&request, &settings(&json!({})), &world.env, Scoring::Off)
    .err()
    .unwrap();
  assert_eq!(failure.code, "plan_invalid");
  assert!(
    failure
      .message
      .starts_with("items file is missing or not JSON: ")
  );
  assert!(on_path(&world.env, "codex"));
  assert!(!on_path(&world.env, "cursor-agent"));
  assert!(!on_path(&Env::from_pairs([("HOME", "/")]), "codex"));
}
