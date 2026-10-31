use toolu_protocol::host::Host;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::env::Env;

use super::super::commit_gate::COMMIT_GATE;
use super::super::docs_sync::DOCS_SYNC;
use super::super::plan_ledger::PLAN_LEDGER;
use super::super::push_review::PUSH_REVIEW;
use super::super::quality_gate::QUALITY_GATE;
use super::super::{command_analysis, gate_config, pre_mode};
use super::EventMemoGuard;
use super::support::{analysis_parses, config_loads, context, reset_counts, shell};
use crate::gate::Gate;
use crate::ledger::context::test_repo::Repo;

#[test]
fn invalid_config_is_loaded_once_and_each_push_gate_keeps_the_warning() {
  let repo = Repo::new().expect("repo");
  let config = repo.root.join(".home/toolu.config.json");
  std::fs::write(&config, "{\"version\":2}\n").expect("config");
  let env = repo.env();
  let (event, raw) = shell("git push origin HEAD:feat/x");
  let ctx = context(&env, &repo.root, &raw, Some(&repo.root), Host::Claude);
  reset_counts();
  let _guard = EventMemoGuard::enter();
  let mut plan_warnings = Vec::new();
  let mut review_warnings = Vec::new();
  COMMIT_GATE.run(&event, &ctx).expect("commit");
  DOCS_SYNC.run(&event, &ctx).expect("docs");
  PLAN_LEDGER
    .run_warning(&event, &ctx, &mut plan_warnings)
    .expect("plan");
  PUSH_REVIEW
    .run_warning(&event, &ctx, &mut review_warnings)
    .expect("review");
  QUALITY_GATE.run(&event, &ctx).expect("quality");
  let loaded = gate_config(&ctx);
  assert_eq!((analysis_parses(), config_loads()), (1, 1));
  assert!(loaded.invalid.is_some(), "{loaded:?}");
  assert_eq!(pre_mode(&loaded, "pushReview", &ctx, true), GateMode::Block);
  assert!(
    plan_warnings
      .iter()
      .any(|line| line.contains("failing closed")),
    "{plan_warnings:?}"
  );
  assert!(
    review_warnings
      .iter()
      .any(|line| line.contains("failing closed")),
    "{review_warnings:?}"
  );
}

#[test]
fn a_new_walk_does_not_reuse_analysis_or_config() {
  let dir = tempfile::tempdir().expect("tempdir");
  let home = dir.path().join("home");
  std::fs::create_dir(&home).expect("home");
  let config = home.join("toolu.config.json");
  std::fs::write(&config, "{\"version\":2}\n").expect("config");
  let home_text = home.to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home_text), ("TOOLU_CONFIG_DIR", home_text)]);
  let (_event, raw) = shell("echo hi");
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  reset_counts();
  {
    let _guard = EventMemoGuard::enter();
    assert_eq!(command_analysis(&ctx).source, "echo hi");
    assert!(gate_config(&ctx).invalid.is_some());
    assert_eq!((analysis_parses(), config_loads()), (1, 1));
  }
  std::fs::write(&config, "{\"not-a-key\":true}\n").expect("rewrite");
  {
    let _guard = EventMemoGuard::enter();
    let (_next, next_raw) = shell("echo next");
    let next_ctx = context(&env, dir.path(), &next_raw, Some(dir.path()), Host::Claude);
    assert_eq!(command_analysis(&next_ctx).source, "echo next");
    let invalid = gate_config(&next_ctx).invalid.expect("invalid");
    assert!(invalid.contains("not-a-key"), "{invalid}");
    assert_eq!((analysis_parses(), config_loads()), (2, 2));
  }
  {
    let _guard = EventMemoGuard::enter();
    let _same = command_analysis(&ctx);
    assert_eq!(analysis_parses(), 3);
  }
}

#[test]
fn host_cwd_and_repository_changes_load_config_again() {
  let root = tempfile::tempdir().expect("tempdir");
  let first = root.path().join("a");
  let second = root.path().join("b");
  std::fs::create_dir(&first).expect("a");
  std::fs::create_dir(&second).expect("b");
  std::fs::write(first.join("toolu.config.json"), "{\"version\":2}\n").expect("a config");
  std::fs::write(second.join("toolu.config.json"), "{\"version\":3}\n").expect("b config");
  let a_text = first.to_str().expect("utf8");
  let b_text = second.to_str().expect("utf8");
  let env_a = Env::from_pairs([
    ("HOME", a_text),
    ("TOOLU_CONFIG_DIR", a_text),
    ("TOOLU_PROJECT_DIR", a_text),
  ]);
  let env_b = Env::from_pairs([
    ("HOME", b_text),
    ("TOOLU_CONFIG_DIR", b_text),
    ("TOOLU_PROJECT_DIR", b_text),
  ]);
  let (_event, raw) = shell("echo hi");
  reset_counts();
  let _guard = EventMemoGuard::enter();
  let claude = context(&env_a, &first, &raw, Some(&first), Host::Claude);
  let loaded = gate_config(&claude);
  assert!(loaded.invalid.is_some());
  let codex = context(&env_a, &first, &raw, Some(&first), Host::Codex);
  assert_eq!(gate_config(&codex).host, Host::Codex);
  let other_cwd = context(&env_a, &first, &raw, Some(&second), Host::Claude);
  let _cwd = gate_config(&other_cwd);
  let other_repo = context(&env_b, &second, &raw, Some(&second), Host::Claude);
  let moved = gate_config(&other_repo);
  assert_eq!(config_loads(), 4, "host, cwd, and repository each load");
  assert!(
    moved.files.user.starts_with(&second),
    "{}",
    moved.files.user.display()
  );
  assert!(
    moved
      .invalid
      .as_deref()
      .is_some_and(|text| text.contains("version")),
    "{moved:?}"
  );
}
