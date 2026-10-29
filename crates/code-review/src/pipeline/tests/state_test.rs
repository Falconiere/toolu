use super::{log_usage, prepass_degraded};
use crate::budget::Effort;
use crate::contract::{Coverage, Report, Usage};
use crate::prepass::{Notice, NoticeKind, PrepassResult};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn usage_report(model: Option<String>) -> Report {
  Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage::default(),
    Usage {
      model,
      ..Usage::default()
    },
  )
}

#[test]
fn failed_scanner_blocks_complete_review_but_overflow_notice_alone_does_not() {
  let mut result = PrepassResult {
    findings: Vec::new(),
    redactions: Vec::new(),
    notices: Vec::new(),
    overflow: false,
  };
  assert!(!prepass_degraded(&result));
  result.notices.push(Notice {
    kind: NoticeKind::AstGrepFailed,
    text: "scanner failed".into(),
  });
  assert!(prepass_degraded(&result));
  result.notices.clear();
  result.notices.push(Notice {
    kind: NoticeKind::Overflow,
    text: "capped".into(),
  });
  assert!(!prepass_degraded(&result));
  result.overflow = true;
  assert!(prepass_degraded(&result));
}

#[test]
fn settle_blocks_an_incomplete_review_and_leaves_a_complete_one_alone() {
  use super::settle;
  use crate::contract::{Omitted, Verdict};

  let mut partial = Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage {
      reviewed: Vec::new(),
      unreviewed: vec![Omitted {
        path: "src/lib.rs".to_owned(),
        reason: "lost".to_owned(),
      }],
      skipped: Vec::new(),
    },
    Usage::default(),
  );
  assert!(!settle(&mut partial));
  assert_eq!(partial.blocking_count(), 1);
  assert_eq!(partial.findings[0].id, "code-review:incomplete");
  let mut whole = Report::reviewed(
    Effort::Medium,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  assert!(settle(&mut whole));
  assert_eq!(whole.verdict(), Verdict::Approved);
  assert_eq!(whole.findings, Vec::new());
}

#[test]
fn usage_retention_bounds_real_rows() {
  use std::process::Command;

  let dir = tempfile::tempdir().unwrap();
  assert!(
    Command::new("git")
      .arg("init")
      .arg("-q")
      .current_dir(dir.path())
      .status()
      .unwrap()
      .success()
  );
  let roots = Roots::new(Env::process(), Some(Host::Codex));
  let state = dir.path().join(".codex/tmp");
  assert!(!crate::shape::dirty_except_state(&Env::process(), dir.path(), &state).unwrap());
  let report = usage_report(Some("model".repeat(20_000)));
  for index in 0..60 {
    log_usage(&roots, dir.path(), &format!("sha-{index}"), &report).unwrap();
  }
  let active = state.join("code-review-usage.jsonl");
  let archives = state.join("code-review-usage");
  let archive_paths: Vec<_> = std::fs::read_dir(archives)
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .collect();
  assert!(archive_paths.len() <= 4);
  assert!(std::fs::metadata(&active).unwrap().len() <= 1_048_576);
  for path in archive_paths.iter().chain(std::iter::once(&active)) {
    let body = std::fs::read_to_string(path).unwrap();
    assert!(std::fs::metadata(path).unwrap().len() <= 1_048_576);
    for line in body.lines() {
      let _: serde_json::Value = serde_json::from_str(line).unwrap();
    }
  }
  assert!(!crate::shape::dirty_except_state(&Env::process(), dir.path(), &state).unwrap());
  std::fs::write(dir.path().join("unrelated.txt"), "dirty").unwrap();
  assert!(crate::shape::dirty_except_state(&Env::process(), dir.path(), &state).unwrap());
}

#[test]
fn usage_failure_on_rotation_or_lock_preserves_the_last_complete_row() {
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(Env::process(), Some(Host::Codex));
  let report = usage_report(Some("m".repeat(1_047_500)));
  log_usage(&roots, dir.path(), "first", &report).unwrap();
  let state = dir.path().join(".codex/tmp");
  let active = state.join("code-review-usage.jsonl");
  let before = std::fs::read(&active).unwrap();
  let archives = state.join("code-review-usage");
  std::fs::write(&archives, "occupied").unwrap();
  let error = log_usage(&roots, dir.path(), "second", &report).unwrap_err();
  assert!(error.contains("usage rotation"), "{error}");
  assert_eq!(std::fs::read(&active).unwrap(), before);
  std::fs::remove_file(archives).unwrap();
  let lock = state.join("code-review-usage.jsonl.lock");
  std::fs::create_dir(&lock).unwrap();
  let error = log_usage(&roots, dir.path(), "third", &report).unwrap_err();
  assert!(error.contains("lock"), "{error}");
  assert_eq!(std::fs::read(active).unwrap(), before);
}

#[test]
fn usage_partial_append_rolls_back_on_a_real_file_limit() {
  if std::env::var_os("TOOLU_USAGE_FILE_LIMIT_CHILD").is_none() {
    let child = std::process::Command::new("sh")
      .args(["-c", "trap '' XFSZ; ulimit -f 1; exec \"$@\"", "sh"])
      .arg(std::env::current_exe().unwrap())
      .arg("usage_partial_append_rolls_back_on_a_real_file_limit")
      .env("TOOLU_USAGE_FILE_LIMIT_CHILD", "1")
      // The low file limit would truncate llvm-cov's exit profile.
      .env("LLVM_PROFILE_FILE", "/dev/null")
      .output()
      .unwrap();
    assert!(
      child.status.success(),
      "{}",
      String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed"));
    return;
  }
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(Env::process(), Some(Host::Codex));
  log_usage(&roots, dir.path(), "before", &usage_report(None)).unwrap();
  let active = dir.path().join(".codex/tmp/code-review-usage.jsonl");
  let before = std::fs::read(&active).unwrap();
  let error = log_usage(
    &roots,
    dir.path(),
    "partial",
    &usage_report(Some("m".repeat(5000))),
  )
  .unwrap_err();
  assert!(error.contains("usage append"), "{error}");
  assert_eq!(std::fs::read(active).unwrap(), before);
}

#[test]
fn usage_cleanup_reports_real_deletion_failure_and_keeps_rows() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join(".codex/tmp");
  let archive_dir = state.join("code-review-usage");
  std::fs::create_dir_all(&archive_dir).unwrap();
  let active = state.join("code-review-usage.jsonl");
  std::fs::write(&active, "{\"diff_sha\":\"active\"}\n").unwrap();
  for sequence in 1..=5 {
    std::fs::write(
      archive_dir.join(format!("{sequence:020}.jsonl")),
      format!("{{\"diff_sha\":\"{sequence}\"}}\n"),
    )
    .unwrap();
  }
  let archives = super::usage_archives(&active).unwrap();
  let oldest = &archives[0].1;
  std::fs::remove_file(oldest).unwrap();
  std::fs::create_dir(oldest).unwrap();
  let error = super::prune_archives(&archives).unwrap_err();
  assert!(error.contains("usage cleanup"), "{error}");
  assert_eq!(
    std::fs::read_to_string(active).unwrap(),
    "{\"diff_sha\":\"active\"}\n"
  );
  for sequence in 2..=5 {
    let row = std::fs::read_to_string(archive_dir.join(format!("{sequence:020}.jsonl"))).unwrap();
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(&row).unwrap()["diff_sha"],
      sequence.to_string()
    );
  }
}

#[test]
fn usage_retention_reclaims_legacy_and_rejects_oversized_row() {
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(Env::process(), Some(Host::Codex));
  let report = usage_report(None);
  let state = dir.path().join(".codex/tmp");
  std::fs::create_dir_all(&state).unwrap();
  let active = state.join("code-review-usage.jsonl");
  std::fs::write(&active, "{}\n".repeat(350_000)).unwrap();
  log_usage(&roots, dir.path(), "current", &report).unwrap();
  assert_eq!(std::fs::read_to_string(&active).unwrap().lines().count(), 1);
  assert_eq!(
    std::fs::read_dir(state.join("code-review-usage"))
      .unwrap()
      .count(),
    0
  );
  let oversized = usage_report(Some("m".repeat(1_048_576)));
  let error = log_usage(&roots, dir.path(), "oversized", &oversized).unwrap_err();
  assert!(error.contains("exceeds 1 MiB"), "{error}");
  assert_eq!(std::fs::read_to_string(active).unwrap().lines().count(), 1);
}

#[test]
fn usage_concurrent_rotation_records_each_success_once() {
  let dir = tempfile::tempdir().unwrap();
  let roots = Roots::new(Env::process(), Some(Host::Codex));
  let large = usage_report(Some("m".repeat(1_047_500)));
  log_usage(&roots, dir.path(), "prefill", &large).unwrap();
  let small = usage_report(None);
  std::thread::scope(|scope| {
    let mut jobs = Vec::new();
    let root = dir.path();
    let roots = &roots;
    let small = &small;
    for index in 0..8 {
      jobs.push(scope.spawn(move || log_usage(roots, root, &format!("sha-{index}"), small)));
    }
    for job in jobs {
      job.join().unwrap().unwrap();
    }
  });
  let state = dir.path().join(".codex/tmp");
  let rows = retained_rows(&state);
  for index in 0..8 {
    assert_eq!(
      rows
        .iter()
        .filter(|row| row["diff_sha"] == format!("sha-{index}"))
        .count(),
      1
    );
  }
}

fn retained_rows(state: &std::path::Path) -> Vec<serde_json::Value> {
  let mut rows = Vec::new();
  for path in [
    state.join("code-review-usage/00000000000000000001.jsonl"),
    state.join("code-review-usage.jsonl"),
  ] {
    for line in std::fs::read_to_string(path).unwrap().lines() {
      rows.push(serde_json::from_str(line).unwrap());
    }
  }
  rows
}

#[test]
fn usage_retention_release_measurement() {
  use std::time::Instant;

  let roots = Roots::new(Env::process(), Some(Host::Codex));
  let report = usage_report(Some("model".repeat(20_000)));
  for sample in 1..=3 {
    let dir = tempfile::tempdir().unwrap();
    let before_fds = std::fs::read_dir("/dev/fd").unwrap().count();
    let start = Instant::now();
    for index in 0..60 {
      log_usage(&roots, dir.path(), &format!("sha-{index}"), &report).unwrap();
    }
    let elapsed = start.elapsed().as_millis();
    let state = dir.path().join(".codex/tmp");
    let mut files = vec![state.join("code-review-usage.jsonl")];
    let archives = state.join("code-review-usage");
    if archives.exists() {
      files.extend(
        std::fs::read_dir(archives)
          .unwrap()
          .map(|entry| entry.unwrap().path()),
      );
    }
    let bytes: u64 = files
      .iter()
      .map(|path| std::fs::metadata(path).unwrap().len())
      .sum();
    let after_fds = std::fs::read_dir("/dev/fd").unwrap().count();
    println!(
      "sample={sample} os={} arch={} writes=60 bytes={bytes} files={} fds_before={before_fds} fds_after={after_fds} elapsed_ms={elapsed}",
      std::env::consts::OS,
      std::env::consts::ARCH,
      files.len()
    );
  }
}
