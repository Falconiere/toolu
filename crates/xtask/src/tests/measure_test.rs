// Resident set and CPU are asserted in `tests/measure.rs` against the real
// binary: in this process, `RUSAGE_CHILDREN` also counts other tests' children.

use nix::sys::time::TimeVal;

use super::{measure, micros, rss_bytes, unsigned};

fn sh(script: &str) -> Vec<String> {
  ["sh", "-c", script].map(str::to_owned).to_vec()
}

#[test]
fn ru_maxrss_is_bytes_on_apple_and_kib_elsewhere() {
  let expected = if cfg!(target_vendor = "apple") {
    2048
  } else {
    2048 * 1024
  };
  assert_eq!(rss_bytes(2048), expected);
  assert_eq!(rss_bytes(u64::MAX), u64::MAX, "saturates");
}

#[test]
fn the_exit_code_is_reported() {
  let report = measure(&sh("exit 3")).unwrap();
  assert_eq!(report.exit_code, Some(3));
  assert_eq!(report.signal, None);
  assert_eq!(report.command, sh("exit 3"));
  assert!(report.wall_us > 0);
}

#[test]
fn a_signal_is_reported_without_an_exit_code() {
  let report = measure(&sh("kill -9 $$")).unwrap();
  assert_eq!(report.exit_code, None);
  assert_eq!(report.signal, Some(9));
}

#[test]
fn no_command_and_an_unspawnable_one_are_errors() {
  assert_eq!(
    measure(&[]).unwrap_err(),
    "measure needs a command after --"
  );
  let err = measure(&["/no/such/command".to_owned()]).unwrap_err();
  assert!(
    err.starts_with("measure: cannot run /no/such/command:"),
    "{err}"
  );
}

#[test]
fn readings_convert_exactly_and_impossible_ones_fail() {
  assert_eq!(unsigned("x", 7_i64), Ok(7));
  assert_eq!(unsigned("wall time", u128::from(u64::MAX)), Ok(u64::MAX));
  // The std error text in parentheses varies by toolchain; the reading itself must be named.
  let too_big = unsigned("wall time", u128::from(u64::MAX) + 1).unwrap_err();
  assert!(
    too_big.starts_with("measure: wall time out of range: 18446744073709551616 ("),
    "{too_big}"
  );
  let negative = unsigned("ru_maxrss", -1_i64).unwrap_err();
  assert!(
    negative.starts_with("measure: ru_maxrss out of range: -1 ("),
    "{negative}"
  );
  assert_eq!(micros("user time", TimeVal::new(2, 500)), Ok(2_000_500));
  assert!(
    micros("user time", TimeVal::new(-1, 0))
      .unwrap_err()
      .starts_with("measure: user time out of range")
  );
  assert!(
    micros("user time", TimeVal::new(i64::MAX, 0))
      .unwrap_err()
      .starts_with("measure: user time out of range")
  );
}
