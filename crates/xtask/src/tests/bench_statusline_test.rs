use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{measure, run, runs, summary, write_result};
use crate::bench_hooks_report::Sample;
use crate::bench_statusline_repo::Fixture;
use crate::bench_statusline_report::Measured;
use crate::options::Options;

fn script(path: &Path, body: &str) -> PathBuf {
  std::fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
  path.to_path_buf()
}

/// A `toolu` that logs its calls and a measurer that really runs the command
/// after `--` (reading the payload from stdin) and reports fixed numbers.
fn tools(dir: &Path, render_exit: u8) -> (PathBuf, PathBuf) {
  let log = dir.join("calls.log");
  let toolu = script(
    &dir.join("toolu"),
    &format!(
      "echo \"$1 $2 $CLAUDE_CONFIG_DIR $PWD\" >> '{}'\ncat > /dev/null\n[ \"$2\" = render ] && exit {render_exit}\nexit 0\n",
      log.display()
    ),
  );
  let measurer = script(
    &dir.join("measurer"),
    "out=$3\nshift 4\n\"$@\"\ncode=$?\nprintf '{\"version\":1,\"command\":[],\"exitCode\":%s,\"signal\":null,\"wallUs\":2000,\"userUs\":100,\"sysUs\":50,\"maxRssBytes\":4000000}' \"$code\" > \"$out\"\n",
  );
  (toolu, measurer)
}

#[test]
fn warm_renders_are_primed_each_and_the_cold_one_uses_the_cold_config() {
  let dir = tempfile::tempdir().unwrap();
  let (toolu, meter) = tools(dir.path(), 0);
  let fixture = Fixture::new().unwrap();
  let measured = measure(&meter, &toolu, &fixture, "{}", 2).unwrap();
  assert_eq!(
    (measured.runs, measured.wall_us, measured.max_rss_bytes),
    (2, 2000, 4_000_000)
  );
  assert_eq!(measured.cold.wall_us, 2000);
  let log = std::fs::read_to_string(dir.path().join("calls.log")).unwrap();
  let lines: Vec<&str> = log.lines().collect();
  assert_eq!(lines.len(), 2 * (3 + 2) + 1, "{log}");
  assert_eq!(
    lines
      .iter()
      .filter(|l| l.starts_with("statusline refresh-cache"))
      .count(),
    5
  );
  let cold = lines.last().unwrap();
  assert!(
    cold.starts_with("statusline render ") && cold.contains("cold-cfg"),
    "{cold}"
  );
}

#[test]
fn a_render_that_fails_is_a_setup_error_not_a_cheap_sample() {
  let dir = tempfile::tempdir().unwrap();
  let (toolu, meter) = tools(dir.path(), 3);
  let fixture = Fixture::new().unwrap();
  let err = measure(&meter, &toolu, &fixture, "{}", 1).unwrap_err();
  assert!(
    err.starts_with("statusline render 1: the hook ended with exit 3"),
    "{err}"
  );
}

#[test]
fn a_missing_toolu_is_a_setup_error_naming_the_build_command() {
  let options = Options {
    root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    bin: Some(PathBuf::from("/no/such/toolu")),
    ..Options::default()
  };
  let err = run(&options).unwrap_err();
  assert!(
    err.starts_with("bench-statusline: no toolu at /no/such/toolu"),
    "{err}"
  );
  assert!(
    err.contains("cargo build --release --locked --bin toolu"),
    "{err}"
  );
}

#[test]
fn runs_default_to_thirty_and_refuse_zero() {
  let mut options = Options::default();
  assert_eq!(runs(&options).unwrap(), 30);
  options.runs = Some("0".to_owned());
  assert_eq!(
    runs(&options).unwrap_err(),
    "--runs takes a whole number above 0, got 0"
  );
}

#[test]
fn the_summary_is_the_nearest_rank_median_of_the_warm_samples() {
  let sample = |wall_us, max_rss_bytes| Sample {
    max_rss_bytes,
    cpu_us: 0,
    wall_us,
  };
  let warm = [sample(30, 9), sample(10, 7), sample(20, 8), sample(40, 6)];
  let measured = summary(&warm, 4, sample(99, 99));
  assert_eq!((measured.wall_us, measured.max_rss_bytes), (20, 7));
  assert_eq!(measured.cold.wall_us, 99);
}

#[test]
fn the_result_is_written_under_missing_directories() {
  let dir = tempfile::tempdir().unwrap();
  let out = dir.path().join("a/b/result.json");
  let cold = Sample {
    max_rss_bytes: 1,
    cpu_us: 0,
    wall_us: 2,
  };
  let measured = Measured {
    runs: 30,
    wall_us: 5,
    max_rss_bytes: 6,
    cold,
  };
  write_result(&out, &measured).unwrap();
  let doc: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
  assert_eq!(doc["schema"], "toolu.statusline-benchmark/v1");
  assert_eq!(doc["coldFirstRender"]["wallUs"], 2);
}
