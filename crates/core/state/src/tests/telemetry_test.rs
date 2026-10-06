use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use super::{
  TELEMETRY_MAX_LINE_BYTES, TelemetryEvent, TelemetryResult, assemble, telemetry_append,
};
use crate::ctx::StateCtx;
use crate::telemetry_schema::parse_telemetry_line;
use crate::time::parse_iso;

/// The `telemetry-events` fixture lines, in document order.
fn fixture_lines() -> Vec<Ordered> {
  let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/state/cases.json");
  let doc = Ordered::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
  let Some(Ordered::Array(cases)) = doc.get("cases") else {
    panic!("no cases")
  };
  let case = cases
    .iter()
    .find(|case| case.get("kind") == Some(&Ordered::String("telemetry-events".into())));
  let Some(Ordered::Array(lines)) = case.unwrap().get("lines") else {
    panic!("no lines")
  };
  lines.clone()
}

/// A project on `branch` with a config, and a context for it fixed at 2026-01-01.
fn sandbox(dir: &Path, branch: &str, config: &str) -> (PathBuf, StateCtx) {
  let project = dir.join("project");
  std::fs::create_dir_all(project.join(".claude")).unwrap();
  std::fs::write(project.join(".claude/toolu.config.json"), config).unwrap();
  for args in [
    &["init", "-q", "-b", branch][..],
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "c",
    ],
  ] {
    let status = Command::new("git")
      .args(args)
      .current_dir(&project)
      .status();
    assert!(status.unwrap().success(), "git {args:?}");
  }
  let env = Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap()),
    ("HOME", dir.join("home").display().to_string()),
  ]);
  let mut ctx = StateCtx::new(Roots::new(env, Some(Host::Claude)));
  ctx.now = parse_iso("2026-01-01T00:00:00.900Z");
  (project, ctx)
}

fn gate_fail() -> TelemetryEvent {
  TelemetryEvent::GateFail {
    file: "/a.ts".into(),
    source: "ts-quality-hook".into(),
  }
}

#[test]
fn every_fixture_line_is_assembled_byte_for_byte() {
  let lines = fixture_lines();
  assert_eq!(lines.len(), 8);
  for line in lines {
    let parsed = parse_telemetry_line(&line).unwrap();
    let ours = assemble(&parsed.event, &parsed.branch, &parsed.t).unwrap();
    assert_eq!(ours, jq_text(&line, false));
  }
}

#[test]
fn a_line_lands_in_the_branch_slug_file_under_the_state_root() {
  let dir = tempfile::tempdir().unwrap();
  let (project, mut ctx) = sandbox(dir.path(), "feat/x", "{\"version\":1}");
  let file = project.join(".claude/tmp/telemetry/feat_x.jsonl");
  assert_eq!(
    telemetry_append(&mut ctx, &project, &gate_fail()),
    TelemetryResult::Written(file.clone())
  );
  assert_eq!(
    telemetry_append(&mut ctx, &project, &TelemetryEvent::DocsNudge),
    TelemetryResult::Written(file.clone())
  );
  let expected = concat!(
    "{\"file\":\"/a.ts\",\"source\":\"ts-quality-hook\",\"v\":1,\"t\":\"2026-01-01T00:00:00Z\",\"branch\":\"feat/x\",\"event\":\"gate_fail\"}\n",
    "{\"v\":1,\"t\":\"2026-01-01T00:00:00Z\",\"branch\":\"feat/x\",\"event\":\"docs_nudge\"}\n",
  );
  assert_eq!(std::fs::read_to_string(file).unwrap(), expected);
  let elsewhere = dir.path().join("elsewhere");
  let roots = Roots::new(
    ctx
      .roots
      .env()
      .clone()
      .with("TELEMETRY_DIR", &elsewhere.display().to_string()),
    Some(Host::Claude),
  );
  ctx.roots = roots;
  assert_eq!(
    telemetry_append(&mut ctx, &project, &gate_fail()),
    TelemetryResult::Written(elsewhere.join("feat_x.jsonl"))
  );
}

#[test]
fn opt_outs_and_unwritable_lines_are_skipped_with_reasons() {
  let dir = tempfile::tempdir().unwrap();
  let (project, mut ctx) = sandbox(
    dir.path(),
    "feat/x",
    "{\"version\":1,\"telemetry\":{\"enabled\":false}}",
  );
  assert_eq!(
    telemetry_append(&mut ctx, &project, &gate_fail()),
    TelemetryResult::Skipped("disabled".into())
  );
  assert_eq!(
    telemetry_append(&mut ctx, Path::new(""), &gate_fail()),
    TelemetryResult::Skipped("no root".into())
  );
  let other = tempfile::tempdir().unwrap();
  let (detached, mut ctx) = sandbox(other.path(), "main", "{\"version\":1}");
  let detach = Command::new("git")
    .args(["checkout", "-q", "--detach"])
    .current_dir(&detached)
    .status();
  assert!(detach.unwrap().success());
  assert_eq!(
    telemetry_append(&mut ctx, &detached, &gate_fail()),
    TelemetryResult::Skipped("no branch".into())
  );
  assert_eq!(ctx.warnings, Vec::<String>::new());
}

#[test]
fn an_oversized_or_non_finite_line_is_refused_with_a_warning() {
  let dir = tempfile::tempdir().unwrap();
  let (project, mut ctx) = sandbox(dir.path(), "feat/x", "{\"version\":1}");
  let big = TelemetryEvent::DocsAttested {
    decision: "x".repeat(TELEMETRY_MAX_LINE_BYTES),
  };
  let TelemetryResult::Skipped(reason) = telemetry_append(&mut ctx, &project, &big) else {
    panic!("written")
  };
  assert!(
    reason.starts_with("telemetry: assembled line for event \"docs_attested\" is "),
    "{reason}"
  );
  let nan = TelemetryEvent::AcCoverage {
    covered: f64::NAN,
    uncovered: 0.0,
  };
  let TelemetryResult::Skipped(reason) = telemetry_append(&mut ctx, &project, &nan) else {
    panic!("written")
  };
  assert_eq!(
    reason,
    "telemetry: invalid extras for event \"ac_coverage\"; skipping append"
  );
  assert_eq!(ctx.warnings.len(), 2);
  std::fs::write(
    project.join(".claude/tmp"),
    "a file where the state dir goes",
  )
  .unwrap();
  let TelemetryResult::Skipped(reason) = telemetry_append(&mut ctx, &project, &gate_fail()) else {
    panic!("written")
  };
  assert!(reason.starts_with("could not append to "), "{reason}");
}

#[test]
fn numbers_print_as_javascript_does() {
  let run = TelemetryEvent::StepRun {
    step_id: "S1".into(),
    status: "green".into(),
    exit_code: 0.0,
    duration_s: 1.5,
    attempt: 2.0,
  };
  let line = assemble(&run, "b", "t").unwrap();
  assert!(
    line.starts_with(
      "{\"step_id\":\"S1\",\"status\":\"green\",\"exit_code\":0,\"duration_s\":1.5,\"attempt\":2,"
    ),
    "{line}"
  );
  let round = TelemetryEvent::PushCheck {
    result: "deny".into(),
    reason_code: "stale".into(),
    round: Some(3.0),
  };
  assert!(assemble(&round, "b", "t").unwrap().contains("\"round\":3,"));
}
