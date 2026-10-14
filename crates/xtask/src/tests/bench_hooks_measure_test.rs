use serde_json::json;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{SPAWN_DEADLINE, Spawn, once, sample_of, samples};
use crate::bench_hooks_report::Sample;

#[test]
fn a_clean_report_is_a_sample_with_user_plus_system_cpu() {
  let report = json!({"version": 1, "command": ["true"], "exitCode": 0, "signal": null,
    "wallUs": 900, "userUs": 300, "sysUs": 200, "maxRssBytes": 4096});
  assert_eq!(
    sample_of(&report).unwrap(),
    Sample {
      max_rss_bytes: 4096,
      cpu_us: 500,
      wall_us: 900
    }
  );
}

#[test]
fn a_failed_or_killed_hook_is_never_a_sample() {
  let failed =
    json!({"exitCode": 2, "signal": null, "wallUs": 1, "userUs": 1, "sysUs": 1, "maxRssBytes": 1});
  assert_eq!(
    sample_of(&failed).unwrap_err(),
    "the hook ended with exit 2 signal null"
  );
  let killed = json!({"exitCode": null, "signal": 9});
  assert_eq!(
    sample_of(&killed).unwrap_err(),
    "the hook ended with exit null signal 9"
  );
  let partial = json!({"exitCode": 0});
  assert_eq!(sample_of(&partial).unwrap_err(), "report lacks maxRssBytes");
}

/// A stand-in `xtask measure`: it is called `measure --out <file> -- <argv>`.
fn measurer(dir: &Path, body: &str) -> PathBuf {
  let file = dir.join("measurer");
  std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
  file
}

fn spawn(dir: &Path, deadline: Duration) -> Spawn<'_> {
  Spawn {
    argv: vec!["true".to_owned()],
    cwd: dir,
    env: vec![("PATH".into(), "/usr/bin:/bin".into())],
    stdin: "{}".to_owned(),
    report_dir: dir,
    deadline,
  }
}

#[test]
fn the_deadline_is_a_minute() {
  assert_eq!(SPAWN_DEADLINE, Duration::from_secs(60));
}

#[test]
fn the_report_is_written_inside_the_report_directory_and_removed() {
  let dir = tempfile::tempdir().unwrap();
  let body =
    r#"printf '%s' '{"exitCode":0,"wallUs":9,"userUs":3,"sysUs":2,"maxRssBytes":4096}' > "$3""#;
  let measurer = measurer(
    dir.path(),
    &format!(
      "{body}\ncase \"$3\" in \"{}\"/*) touch \"{}/inside\";; esac",
      dir.path().display(),
      dir.path().display()
    ),
  );
  let kept = samples(
    &measurer,
    &spawn(dir.path(), SPAWN_DEADLINE),
    "entry",
    (1, 2),
  )
  .unwrap();
  assert_eq!(kept.len(), 2);
  assert_eq!(kept[0].cpu_us, 5);
  assert!(dir.path().join("inside").is_file());
  assert!(!dir.path().join("measure-0.json").exists());
}

#[test]
fn a_measurer_past_its_deadline_is_killed_with_its_group_and_names_the_entry() {
  let dir = tempfile::tempdir().unwrap();
  let measurer = measurer(dir.path(), "echo stuck >&2\nsleep 30 &\nsleep 30");
  let started = Instant::now();
  let err = once(
    &measurer,
    &spawn(dir.path(), Duration::from_secs(2)),
    "toolu/slow-hook",
    0,
  )
  .unwrap_err();
  assert!(
    err.starts_with("toolu/slow-hook: the measurer ran past 2 s and was killed: stuck"),
    "{err}"
  );
  assert!(started.elapsed() < Duration::from_secs(10), "{err}");
}

#[test]
fn a_failing_measurer_reports_its_exit_and_stderr() {
  let dir = tempfile::tempdir().unwrap();
  let measurer = measurer(dir.path(), "echo cannot measure >&2\nexit 4");
  let err = once(&measurer, &spawn(dir.path(), SPAWN_DEADLINE), "entry", 0).unwrap_err();
  assert_eq!(err, "entry: measurer exited Some(4): cannot measure");
}
