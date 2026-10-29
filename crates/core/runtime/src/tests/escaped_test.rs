//! Real escaped descendants that keep one inherited pipe open after their leader exits.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{Spec, Wait, group, run, run_with_chunks};

struct Escape {
  _dir: tempfile::TempDir,
  pidfile: std::path::PathBuf,
}

impl Escape {
  fn new(mode: &str) -> (Self, Spec) {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("escaped.pid");
    let redirect = match mode {
      "stdin" => "<&0 >/dev/null 2>&1",
      "stdout" => "</dev/null 2>/dev/null",
      "stderr" => "</dev/null >/dev/null",
      _ => panic!("unknown pipe mode"),
    };
    let script = format!(
      "perl -MPOSIX -e 'POSIX::setsid() or die; open my $f, \">\", $ARGV[0] or die; print $f \"$$\\n\"; close $f; sleep 30' \"$1\" {redirect} & while [ ! -s \"$1\" ]; do sleep 0.01; done; exit 0"
    );
    let mut spec = Spec::new(["sh", "-c", &script, "sh", &pidfile.to_string_lossy()]);
    spec.timeout = Duration::from_millis(150);
    if mode == "stdin" {
      spec.stdin = vec![b'x'; 8 * 1024 * 1024];
    }
    (Self { _dir: dir, pidfile }, spec)
  }

  fn pid(&self) -> u32 {
    std::fs::read_to_string(&self.pidfile)
      .unwrap()
      .trim()
      .parse()
      .unwrap()
  }
}

impl Drop for Escape {
  fn drop(&mut self) {
    if let Ok(pid) = std::fs::read_to_string(&self.pidfile)
      && let Ok(pid) = pid.trim().parse::<u32>()
    {
      if group::pid_alive(i64::from(pid)).unwrap_or(false) {
        let _ = group::signal_pid(pid, group::Signal::SIGTERM);
      }
      std::thread::sleep(Duration::from_millis(20));
      if group::pid_alive(i64::from(pid)).unwrap_or(false) {
        let _ = group::signal_pid(pid, group::Signal::SIGKILL);
      }
    }
  }
}

fn resource_counts() -> (usize, usize) {
  #[cfg(target_os = "linux")]
  let threads = std::fs::read_dir("/proc/self/task").unwrap().count();
  #[cfg(target_os = "macos")]
  let threads = {
    let output = std::process::Command::new("ps")
      .args(["-M", "-p", &std::process::id().to_string()])
      .output()
      .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
      .lines()
      .count()
      .saturating_sub(1)
  };
  #[cfg(target_os = "linux")]
  let fds = std::fs::read_dir("/proc/self/fd").unwrap().count();
  #[cfg(target_os = "macos")]
  let fds = std::fs::read_dir("/dev/fd").unwrap().count();
  (threads, fds)
}

fn assert_resources_settle(baseline: (usize, usize)) {
  let deadline = Instant::now() + Duration::from_millis(300);
  loop {
    let actual = resource_counts();
    if actual.0 <= baseline.0 && actual.1 <= baseline.1 {
      return;
    }
    assert!(
      Instant::now() < deadline,
      "baseline={baseline:?}, retained={actual:?}"
    );
    std::thread::sleep(Duration::from_millis(10));
  }
}

fn escaped_case(mode: &str) {
  let baseline = resource_counts();
  let (escape, spec) = Escape::new(mode);
  let capture = Arc::new(());
  let captured = Arc::clone(&capture);
  let started = Instant::now();
  let output = run_with_chunks(
    &spec,
    &mut |_| Ok(()),
    Arc::new(move |_| {
      std::hint::black_box(&captured);
    }),
  )
  .unwrap();
  assert!(started.elapsed() < Duration::from_secs(3));
  assert!(group::pid_alive(i64::from(escape.pid())).unwrap());
  assert!(!group::alive(output.pid));
  assert_resources_settle(baseline);
  assert_eq!(Arc::strong_count(&capture), 1, "{mode} retained observer");
  eprintln!(
    "{mode}: baseline={baseline:?}, settled={:?}",
    resource_counts()
  );
}

#[test]
fn escaped_stdin_releases_pipe_worker_before_descendant_cleanup() {
  escaped_case("stdin");
}

#[test]
fn escaped_output_releases_each_drain_and_observer_before_descendant_cleanup() {
  escaped_case("stdout");
  escaped_case("stderr");
}

#[test]
fn escaped_resources_remain_stable_across_repeated_runs() {
  let baseline = resource_counts();
  for _ in 0..5 {
    for mode in ["stdin", "stdout", "stderr"] {
      escaped_case(mode);
      assert_resources_settle(baseline);
    }
  }
}

#[test]
fn escaped_slow_reader_receives_full_stdin_before_deadline() {
  let dir = tempfile::tempdir().unwrap();
  let pidfile = dir.path().join("escaped.pid");
  let countfile = dir.path().join("count");
  let escape = Escape {
    _dir: dir,
    pidfile: pidfile.clone(),
  };
  let script = "perl -MPOSIX -e 'POSIX::setsid() or die; open my $f, \">\", $ARGV[0] or die; print $f \"$$\\n\"; close $f; select undef, undef, undef, 0.35; my $n = 0; while (my $r = sysread(STDIN, my $b, 8192)) { $n += $r }; open my $out, \">\", $ARGV[1] or die; print $out $n; close $out' \"$1\" \"$2\" <&0 >/dev/null 2>&1 & while [ ! -s \"$1\" ]; do sleep 0.01; done; exit 0";
  let mut spec = Spec::new([
    "sh",
    "-c",
    script,
    "sh",
    &pidfile.to_string_lossy(),
    &countfile.to_string_lossy(),
  ]);
  spec.stdin = vec![b'x'; 1024 * 1024];
  spec.timeout = Duration::from_secs(2);
  spec.wait = Wait::Streams;
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0);
  assert!(!output.timed_out);
  let deadline = Instant::now() + Duration::from_secs(1);
  while !countfile.exists() && Instant::now() < deadline {
    std::thread::sleep(Duration::from_millis(10));
  }
  assert_eq!(std::fs::read_to_string(countfile).unwrap(), "1048576");
  drop(escape);
}
