use std::path::Path;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Settings, load};
use crate::error::Failure;
use crate::fixer::host::{Choice, Fixer};

struct World {
  _dir: TempDir,
  user: std::path::PathBuf,
  project: std::path::PathBuf,
  roots: Roots,
}

fn world() -> World {
  let dir = tempfile::tempdir().unwrap();
  let user = dir.path().join("user");
  let project = dir.path().join("project");
  std::fs::create_dir_all(user.join(".claude")).unwrap();
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  let env = Env::from_pairs([
    ("HOME", user.to_str().unwrap()),
    ("TOOLU_PROJECT_DIR", project.to_str().unwrap()),
  ]);
  let roots = Roots::new(env, Some(Host::Claude));
  World {
    _dir: dir,
    user: user.join(".claude/toolu.config.json"),
    project: project.join(".claude/toolu.config.json"),
    roots,
  }
}

fn write(path: &Path, document: &Value) {
  std::fs::write(path, document.to_string()).unwrap();
}

fn settings(world: &World) -> Result<Settings, Failure> {
  load(&world.roots, None, Fixer::Claude)
}

fn message(world: &World) -> String {
  let failure = settings(world).err().unwrap();
  assert_eq!(failure.code, "config_invalid");
  failure.message
}

#[test]
fn without_config_the_controller_host_fixes_with_default_routing() {
  let world = world();
  let loaded = settings(&world).unwrap();
  assert_eq!(loaded.hosts, [Fixer::Claude]);
  assert!(!loaded.inline && loaded.unattended && loaded.jev);
  assert_eq!(loaded.prefer.len(), 0);
  assert_eq!(loaded.routing[&Fixer::Codex], Fixer::Codex.defaults());
}

#[test]
fn the_project_block_merges_over_the_user_block_field_by_field() {
  let world = world();
  write(
    &world.user,
    &json!({"prBabysit": {"dispatch": "inline", "hosts": ["codex"], "jev": false}}),
  );
  let routing = json!([{}, {}, {}, {"model": "opus", "effort": null}]);
  write(
    &world.project,
    &json!({"prBabysit": {
      "dispatch": "herdr",
      "hosts": ["cursor-agent", "Cursor", "claude-code"],
      "unattended": false,
      "prefer": {"critical": ["claude"]},
      "routing": {"claude": routing}
    }}),
  );
  let loaded = settings(&world).unwrap();
  assert!(!loaded.inline && !loaded.unattended && !loaded.jev);
  assert_eq!(loaded.hosts, [Fixer::Cursor, Fixer::Claude]);
  assert_eq!(loaded.prefer[&3], [Fixer::Claude]);
  let critical = Choice {
    model: Some("opus".into()),
    effort: None,
  };
  assert_eq!(loaded.routing[&Fixer::Claude][3], critical);
  assert_eq!(loaded.routing[&Fixer::Claude][0], Choice::default());
}

#[test]
fn muse_is_a_fixer_host_whose_routing_can_be_overridden() {
  let world = world();
  let routing = json!([{}, {}, {}, {"model": "muse-wonder-full-1.0", "effort": "high"}]);
  write(
    &world.project,
    &json!({"prBabysit": {
      "hosts": ["claude", "Muse"],
      "prefer": {"critical": ["muse"]},
      "routing": {"muse": routing}
    }}),
  );
  let loaded = settings(&world).unwrap();
  assert_eq!(loaded.hosts, [Fixer::Claude, Fixer::Muse]);
  assert_eq!(loaded.prefer[&3], [Fixer::Muse]);
  let critical = Choice {
    model: Some("muse-wonder-full-1.0".into()),
    effort: Some("high".into()),
  };
  assert_eq!(loaded.routing[&Fixer::Muse][3], critical);
}

#[test]
fn malformed_files_and_files_without_the_block_are_ignored() {
  let world = world();
  std::fs::write(&world.user, "{not json").unwrap();
  write(&world.project, &json!(["prBabysit"]));
  assert_eq!(settings(&world).unwrap().hosts, [Fixer::Claude]);
}

#[test]
fn each_invalid_field_fails_with_its_own_message() {
  let cases = [
    (json!(false), "prBabysit must be an object (in "),
    (
      json!({"dispatch": "tmux"}),
      "prBabysit: dispatch must be herdr or inline",
    ),
    (
      json!({"hosts": []}),
      "prBabysit: hosts must name at least one host",
    ),
    (
      json!({"hosts": "codex"}),
      "prBabysit: hosts must be an array of hosts",
    ),
    (json!({"hosts": ["gemini"]}), "unknown host 'gemini'"),
    (json!({"prefer": []}), "prBabysit: prefer must be an object"),
    (
      json!({"prefer": {"huge": ["codex"]}}),
      "prBabysit: prefer key huge is not a tier",
    ),
    (
      json!({"routing": 1}),
      "prBabysit: routing must be an object",
    ),
    (
      json!({"routing": {"codex": [{}]}}),
      "prBabysit: routing.codex must list 4 tiers",
    ),
    (
      json!({"routing": {"muse": [{}, {}]}}),
      "prBabysit: routing.muse must list 4 tiers",
    ),
    (
      json!({"routing": {"claude": [{"model": "a b"}, {}, {}, {}]}}),
      "prBabysit: routing.claude has a non-string or shell-unsafe model/effort",
    ),
    (
      json!({"unattended": "yes"}),
      "prBabysit: unattended must be true or false",
    ),
    (json!({"jev": 0}), "prBabysit: jev must be true or false"),
  ];
  let world = world();
  for (block, expected) in cases {
    write(&world.project, &json!({ "prBabysit": block }));
    let message = message(&world);
    assert!(message.starts_with(expected), "{message} vs {expected}");
  }
}
