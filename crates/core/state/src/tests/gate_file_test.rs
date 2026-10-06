use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{
  ClearOutcome, GateFailure, GateRead, clear_gate_file, read_gate_file, record_gate_failure,
};
use crate::ctx::StateCtx;
use crate::gate_schema::GateFile;
use crate::lock::lock_path;
use crate::time::parse_iso;

/// A git project on `feat/x` with its gate path, and a context at 2026-09-28T10:00:00Z.
fn gate_project(dir: &Path, name: &str) -> (PathBuf, PathBuf, StateCtx) {
  let project = dir.join("project");
  std::fs::create_dir_all(project.join(".claude/tmp")).unwrap();
  for args in [
    &["init", "-q", "-b", "feat/x"][..],
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
    assert!(
      Command::new("git")
        .args(args)
        .current_dir(&project)
        .status()
        .unwrap()
        .success()
    );
  }
  let env = Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap()),
    ("HOME", dir.join("home").display().to_string()),
  ]);
  let mut ctx = StateCtx::new(Roots::new(env, Some(Host::Claude)));
  ctx.now = parse_iso("2026-09-28T10:00:00.500Z");
  let gate = project.join(".claude/tmp").join(name);
  (project, gate, ctx)
}

fn record(ctx: &mut StateCtx, gate: &Path, [file, source, reason, violations]: [&str; 4]) {
  record_gate_failure(
    ctx,
    gate,
    &GateFailure {
      file,
      source,
      reason,
      violations,
    },
  );
}

fn read(path: &Path) -> String {
  std::fs::read_to_string(path).unwrap()
}

#[test]
fn reads_classify_missing_malformed_unrecognized_and_valid() {
  let dir = tempfile::tempdir().unwrap();
  let gate = dir.path().join("gate.json");
  assert_eq!(read_gate_file(&gate), GateRead::Missing);
  for body in ["", "{oops", "null", "false"] {
    std::fs::write(&gate, body).unwrap();
    assert!(
      matches!(read_gate_file(&gate), GateRead::Malformed(_)),
      "{body:?}"
    );
  }
  std::fs::write(&gate, "[]").unwrap();
  assert!(matches!(
    read_gate_file(&gate),
    GateRead::Unrecognized { .. }
  ));
  std::fs::write(
    &gate,
    r#"{"status":"passing","source":"s","updatedAt":"u"}"#,
  )
  .unwrap();
  assert!(matches!(
    read_gate_file(&gate),
    GateRead::Ok(GateFile::Passing { .. })
  ));
  std::fs::remove_file(&gate).unwrap();
  std::fs::create_dir(&gate).unwrap();
  assert!(
    matches!(read_gate_file(&gate), GateRead::Malformed(_)),
    "a directory"
  );
}

#[test]
fn records_join_their_violations_oldest_first() {
  let dir = tempfile::tempdir().unwrap();
  let (_, gate, mut ctx) = gate_project(dir.path(), "quality-gate-status.json");
  record(
    &mut ctx,
    &gate,
    ["/r/a.ts", "ts", "too long", "a.ts: 400 lines\n"],
  );
  record(
    &mut ctx,
    &gate,
    ["/r/b.ts", "ts", "too long", "b.ts: 500 lines\n"],
  );
  let GateRead::Ok(GateFile::Failing { violations, .. }) = read_gate_file(&gate) else {
    panic!()
  };
  assert_eq!(violations, "a.ts: 400 lines\nb.ts: 500 lines\n");
  assert_eq!(ctx.warnings, Vec::<String>::new());
  assert!(!lock_path(&gate).exists());
}

#[test]
fn clears_reach_passing_and_every_write_leaves_telemetry() {
  let dir = tempfile::tempdir().unwrap();
  let (project, gate, mut ctx) = gate_project(dir.path(), "quality-gate-status.json");
  record(&mut ctx, &gate, ["/r/a.ts", "ts", "r", "a\n"]);
  record(&mut ctx, &gate, ["/r/b.ts", "ts", "r", "b\n"]);
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/r/a.ts", "other"),
    ClearOutcome::Noop
  );
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/r/a.ts", "ts"),
    ClearOutcome::Cleared
  );
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/r/b.ts", "ts"),
    ClearOutcome::Cleared
  );
  let passing = "{\n  \"status\": \"passing\",\n  \"source\": \"ts\",\n  \"updatedAt\": \"2026-09-28T10:00:00Z\"\n}\n";
  assert_eq!(read(&gate), passing);
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/r/b.ts", "ts"),
    ClearOutcome::Noop
  );
  let log = read(&project.join(".claude/tmp/telemetry/feat_x.jsonl"));
  let fails = log
    .lines()
    .filter(|line| line.ends_with("\"event\":\"gate_fail\"}"))
    .count();
  let clears = log
    .lines()
    .filter(|line| line.ends_with("\"event\":\"gate_clear\"}"))
    .count();
  assert_eq!((fails, clears), (2, 2));
}

#[test]
fn an_unrecognized_document_is_replaced_on_record_and_kept_on_clear() {
  let dir = tempfile::tempdir().unwrap();
  let (_, gate, mut ctx) = gate_project(dir.path(), "quality-gate-status.json");
  let foreign = r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v","entries":{"/a":{"source":"s","reason":"r","violations":"v","updatedAt":"u"},"/b":{"source":"s","reason":"r","violations":"v","updatedAt":"u"}},"updatedAt":"u","owner":"x"}"#;
  std::fs::write(&gate, foreign).unwrap();
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/a", "s"),
    ClearOutcome::Noop
  );
  assert_eq!(read(&gate), foreign);
  assert!(
    ctx.warnings[0].ends_with("; ignoring clear"),
    "{:?}",
    ctx.warnings
  );
  record(&mut ctx, &gate, ["/c", "s", "r", "c\n"]);
  assert!(ctx.warnings[1].starts_with(&format!(
    "gate-file: unrecognized gate file at {} (",
    gate.display()
  )));
  let log = read(&PathBuf::from(format!("{}.dropped.log", gate.display())));
  assert_eq!(
    log,
    "2026-09-28T10:00:00Z unrecognized gate file replaced; dropped 2 entry(ies)\n"
  );
  std::fs::write(&gate, "{oops").unwrap();
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/c", "s"),
    ClearOutcome::Noop
  );
  let malformed = format!(
    "gate-file: malformed JSON at {}; ignoring clear (gate stays failing until next write)",
    gate.display()
  );
  assert_eq!(ctx.warnings.last(), Some(&malformed));
}

#[test]
fn a_failed_atomic_write_falls_back_to_a_single_slot_record() {
  let dir = tempfile::tempdir().unwrap();
  // The temp name (`<name>.<random>.tmp`) passes the 255-byte name limit; the gate does not.
  let name = format!("{}.json", "q".repeat(245));
  let (_, gate, mut ctx) = gate_project(dir.path(), &name);
  let two = r#"{"status":"failing","reason":"r","source":"s","file":"/a","violations":"v","entries":{"/a":{"source":"s","reason":"r","violations":"v","updatedAt":"u"},"/b":{"source":"s","reason":"r","violations":"v","updatedAt":"u"}},"updatedAt":"u"}"#;
  std::fs::write(&gate, two).unwrap();
  record(&mut ctx, &gate, ["/c", "s", "r", "c\n"]);
  let GateRead::Ok(GateFile::Failing { file, entries, .. }) = read_gate_file(&gate) else {
    panic!()
  };
  assert_eq!((file.as_str(), entries), ("/c", None));
  assert_eq!(ctx.warnings.len(), 1);
  assert!(
    ctx.warnings[0].ends_with("single-slot fallback dropped 2 other entry(ies)"),
    "{:?}",
    ctx.warnings
  );
}

#[test]
fn a_clear_with_nothing_to_clear_never_waits_on_a_live_lock() {
  let dir = tempfile::tempdir().unwrap();
  let (_, gate, mut ctx) = gate_project(dir.path(), "quality-gate-status.json");
  std::fs::write(
    &gate,
    r#"{"status":"passing","source":"s","updatedAt":"u"}"#,
  )
  .unwrap();
  let lock = format!("{} busy\n", std::process::id());
  std::fs::write(lock_path(&gate), &lock).unwrap();
  let started = std::time::Instant::now();
  assert_eq!(
    clear_gate_file(&mut ctx, &gate, "/a", "s"),
    ClearOutcome::Noop
  );
  assert!(started.elapsed() < std::time::Duration::from_millis(500));
  assert_eq!(read(&lock_path(&gate)), lock);
}
