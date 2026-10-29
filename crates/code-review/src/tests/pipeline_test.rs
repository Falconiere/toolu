use std::path::Path;

use serde_json::json;
use toolu_engine::verdict::gates::GateContext;
use toolu_engine::verdict::review::review_gate;
use toolu_http_test_support::{Fixture, Reply};
use toolu_protocol::host::Host;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::push_review::{Input as StateInput, write as write_state};

use super::run;
use crate::cli;
use crate::contract::{ErrorKind, Verdict};
use crate::review::{Call, Options};

mod fixture_test;
use fixture_test::{clean_case, env, finding_case, invoke, options, repo, saved_state};

#[test]
fn default_enabled_real_git_diff_writes_gate_spelled_paths_and_metadata_only_usage() {
  let (repo, state, fixture) = clean_case();
  let env = env(repo.path(), state.path(), &fixture);
  let report = invoke(&env, &options(&fixture));
  assert_eq!(report.verdict(), Verdict::Approved, "{report:?}");
  let call: &Call = &report.usage.calls[0];
  assert_eq!(call.provider.as_deref(), Some("Parasail"));
  assert_eq!(call.quantization.as_deref(), Some("fp8"));
  let file = state.path().join("feature.json");
  let saved = std::fs::read_to_string(&file).unwrap();
  let doc: serde_json::Value = serde_json::from_str(&saved).unwrap();
  assert_eq!(
    doc["reviewed_files"],
    json!([r#""caf\303\251.rs""#, "src.txt"])
  );
  let roots = Roots::new(
    env.clone().with("PUSH_REVIEW_BASE", "main"),
    Some(Host::Codex),
  );
  let gate = GateContext {
    roots: &roots,
    root: repo.path().to_path_buf(),
    branch: "feature".into(),
    base: "main".into(),
    cur: doc["diff_sha"].as_str().unwrap().into(),
    cwd: repo.path().to_path_buf(),
    warnings: Vec::new(),
  };
  assert_eq!(
    review_gate(&gate).get("state"),
    Some(&toolu_runtime::json::ordered::Ordered::String(
      "pass".into()
    ))
  );
  let usage =
    std::fs::read_to_string(repo.path().join(".codex/tmp/code-review-usage.jsonl")).unwrap();
  assert!(
    usage.contains("Parasail") && usage.contains("fp8") && usage.contains(r#""outcome":"ok""#)
  );
  assert!(!usage.contains("private-sentinel-669") && !usage.contains("pub fn value"));
  assert_eq!(
    invoke(&env, &options(&fixture)).verdict(),
    Verdict::Approved
  );
}

#[test]
fn model_outage_keeps_mechanical_checks_and_blocks_gate() {
  let repo = repo();
  let state = tempfile::tempdir().unwrap();
  let fixture = Fixture::start().unwrap();
  fixture
    .route("/api/v1/chat/completions", Reply::new(503, "down"))
    .unwrap();
  let env = env(repo.path(), state.path(), &fixture);
  let report = invoke(&env, &options(&fixture));
  assert_eq!(report.verdict(), Verdict::Error);
  assert!(
    report
      .findings
      .iter()
      .any(|finding| finding.severity.blocking())
  );
  assert_ne!(report.mechanical_checks.len(), 0);
  let saved: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(state.path().join("feature.json")).unwrap())
      .unwrap();
  assert!(saved["findings_count"].as_u64().unwrap() > 0);
}

#[test]
fn state_write_failure_keeps_paid_review_findings_in_report() {
  let (repo, state, fixture) = finding_case(false);
  std::fs::create_dir(state.path().join("feature.json")).unwrap();
  let env = env(repo.path(), state.path(), &fixture);
  let report = invoke(
    &env,
    &Options {
      skip_judge: true,
      ..options(&fixture)
    },
  );
  assert_eq!(report.verdict(), Verdict::Error);
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.findings.len(), 1);
  assert_ne!(report.usage.calls.len(), 0);
}

#[test]
fn failed_usage_append_and_rotation_replace_an_older_passing_gate_state() {
  let (repo, state, fixture) = clean_case();
  let env = env(repo.path(), state.path(), &fixture);
  assert_eq!(
    invoke(&env, &options(&fixture)).verdict(),
    Verdict::Approved
  );
  let log = repo.path().join(".codex/tmp/code-review-usage.jsonl");
  std::fs::remove_file(&log).unwrap();
  std::fs::create_dir(&log).unwrap();
  let report = invoke(&env, &options(&fixture));
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.blocking_count(), 1);
  assert_eq!(
    saved_state(state.path())["findings_count"],
    report.blocking_count()
  );
  std::fs::remove_dir(&log).unwrap();
  std::fs::write(&log, "{}\n".repeat(350_000)).unwrap();
  std::fs::write(repo.path().join(".codex/tmp/code-review-usage"), "occupied").unwrap();
  let report = invoke(&env, &options(&fixture));
  assert_eq!(
    report.error.as_ref().map(|error| error.kind),
    Some(ErrorKind::State)
  );
  assert_eq!(report.blocking_count(), 1);
  assert_eq!(saved_state(state.path())["findings_count"], 1);
}

#[test]
fn a_model_approved_verdict_with_a_high_finding_is_blocked() {
  let (repo, state, fixture) = finding_case(true);
  let env = env(repo.path(), state.path(), &fixture);
  let options = Options {
    skip_judge: true,
    ..options(&fixture)
  };
  let report = invoke(&env, &options);
  assert_eq!(report.verdict(), Verdict::Blocked);
  assert_eq!(report.blocking_count(), 1);
  let saved = saved_state(state.path());
  assert_eq!(saved["findings_count"], 1);
}

#[test]
fn a_path_scoped_clean_response_cannot_write_passing_state() {
  let (repo, state, fixture) = clean_case();
  let env = env(repo.path(), state.path(), &fixture);
  let matches = cli::command()
    .try_get_matches_from([
      "code-review",
      "run",
      "--base",
      "main",
      "--effort",
      "medium",
      "--path",
      "src.txt",
    ])
    .unwrap();
  let ctx = Ctx {
    host: Some(Host::Codex),
    ..Ctx::default()
  };
  let report = run(
    &env,
    &ctx,
    matches.subcommand_matches("run").unwrap(),
    &options(&fixture),
  )
  .unwrap();
  assert_eq!(report.verdict(), Verdict::Blocked);
  assert_ne!(report.coverage.unreviewed.len(), 0);
  let saved = saved_state(state.path());
  assert!(saved["findings_count"].as_u64().unwrap() > 0);
}

#[test]
fn a_path_scope_narrows_the_packed_chunk_instead_of_splitting_it() {
  let repo = repo();
  let git = Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null");
  let mut shaped = crate::shape::shape(&git, repo.path(), Some("main")).unwrap();
  assert_eq!(shaped.chunks.len(), 1);
  super::scope(&mut shaped, &["src.txt"]);
  let index = shaped.files.iter().position(|file| file.path == "src.txt");
  assert_eq!(shaped.chunks.len(), 1);
  assert_eq!(shaped.chunks[0].files, [index.unwrap()]);
  assert_eq!(
    shaped.chunks[0].bytes,
    shaped.files[index.unwrap()].patch.len()
  );
}

#[test]
fn dirty_worktree_cannot_write_passing_state() {
  let (repo, state, fixture) = clean_case();
  std::fs::write(repo.path().join("src.txt"), "uncommitted\n").unwrap();
  let env = env(repo.path(), state.path(), &fixture);
  let report = invoke(&env, &options(&fixture));
  assert_eq!(report.verdict(), Verdict::Blocked);
  assert_eq!(report.blocking_count(), 1);
  assert!(
    report
      .coverage
      .unreviewed
      .iter()
      .any(|item| item.reason == "worktree has uncommitted changes")
  );
  assert!(
    saved_state(state.path())["findings_count"]
      .as_u64()
      .unwrap()
      > 0
  );
}

#[test]
fn the_other_writer_preserves_memory_and_round_six_escalates() {
  let (repo, state, fixture) = clean_case();
  let env = env(repo.path(), state.path(), &fixture).with("PUSH_REVIEW_BASE", "main");
  let report = invoke(&env, &options(&fixture));
  assert_eq!(report.verdict(), Verdict::Approved);
  let memory_path = repo.path().join(".codex/tmp/code-review/feature.json");
  let before = std::fs::read_to_string(&memory_path).unwrap();
  let root = repo.path().to_str().unwrap();
  let review = StateInput {
    findings_count: 0,
    reviewers: Ordered::parse("[\"toolu-review:review\"]").unwrap(),
    findings: Ordered::parse("[]").unwrap(),
    repo: root,
    branch: None,
    reviewed_files: None,
    expected_diff_sha: None,
  };
  let ctx = Ctx {
    host: Some(Host::Codex),
    ..Ctx::default()
  };
  for _ in 0..5 {
    write_state(&review, &ctx, &env).unwrap();
  }
  let saved = saved_state(state.path());
  assert_eq!(saved["review_round"], 6);
  assert_eq!(saved["reviewers"], json!(["toolu-review:review"]));
  assert_eq!(std::fs::read_to_string(memory_path).unwrap(), before);
  assert_escalated(env, repo.path(), saved["diff_sha"].as_str().unwrap());
}

fn assert_escalated(env: Env, root: &Path, diff_sha: &str) {
  let roots = Roots::new(env, Some(Host::Codex));
  let gate = GateContext {
    roots: &roots,
    root: root.to_path_buf(),
    branch: "feature".into(),
    base: "main".into(),
    cur: diff_sha.into(),
    cwd: root.to_path_buf(),
    warnings: Vec::new(),
  };
  assert_eq!(
    review_gate(&gate).get("state"),
    Some(&Ordered::String("escalate".into()))
  );
  assert_eq!(
    review_gate(&gate).get("reason_code"),
    Some(&Ordered::String("round-cap".into()))
  );
}

#[test]
fn exact_dismissal_refreshes_gate_without_another_model_call() {
  let (repo, state, fixture) = finding_case(false);
  let env = env(repo.path(), state.path(), &fixture);
  let options = Options {
    skip_judge: true,
    ..options(&fixture)
  };
  let report = invoke(&env, &options);
  assert_eq!(report.verdict(), Verdict::Blocked);
  let id = report.findings[0].id.clone();
  let count = fixture.requests().unwrap().len();
  let ctx = Ctx {
    host: Some(Host::Codex),
    ..Ctx::default()
  };
  let message = super::dismiss(&env, &ctx, &id, "guarded by the caller").unwrap();
  assert!(message.contains("state refreshed"), "{message}");
  assert_eq!(fixture.requests().unwrap().len(), count);
  let saved = saved_state(state.path());
  assert_eq!(saved["findings_count"], 0);
  let memory =
    std::fs::read_to_string(repo.path().join(".codex/tmp/code-review/feature.json")).unwrap();
  assert!(memory.contains("guarded by the caller"));
}
