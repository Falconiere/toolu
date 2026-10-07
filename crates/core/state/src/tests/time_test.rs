use std::time::{Duration, UNIX_EPOCH};

use super::{iso_seconds, parse_iso};

#[test]
fn iso_seconds_truncates_to_the_second_in_utc() {
  let t = parse_iso("2026-09-28T12:34:56.789Z").unwrap();
  assert_eq!(iso_seconds(t), "2026-09-28T12:34:56Z");
  assert_eq!(iso_seconds(UNIX_EPOCH), "1970-01-01T00:00:00Z");
  assert_eq!(
    iso_seconds(parse_iso("2024-02-29T23:59:59Z").unwrap()),
    "2024-02-29T23:59:59Z"
  );
  assert_eq!(
    iso_seconds(parse_iso("2000-03-01T00:00:00Z").unwrap()),
    "2000-03-01T00:00:00Z"
  );
}

#[test]
fn before_the_epoch_seconds_round_down() {
  let t = UNIX_EPOCH - Duration::from_millis(500);
  assert_eq!(iso_seconds(t), "1969-12-31T23:59:59Z");
  assert_eq!(parse_iso("1969-12-31T23:59:59.500Z"), Some(t));
}

#[test]
fn parse_reads_milliseconds_and_rejects_other_shapes() {
  let t = parse_iso("2026-09-28T10:00:00.5Z").unwrap();
  assert_eq!(t.duration_since(UNIX_EPOCH).unwrap().subsec_millis(), 500);
  for bad in [
    "2026-09-28T10:00:00",
    "2026-09-28 10:00:00Z",
    "2026-09T10:00:00Z",
    "xZ",
  ] {
    assert_eq!(parse_iso(bad), None, "{bad}");
  }
  assert_eq!(parse_iso("2026-09-28T10:00:00:01Z"), None);
  for out_of_range in [
    "2026-13-01T00:00:00Z",
    "2026-01-32T00:00:00Z",
    "2026-01-01T24:00:00Z",
    "2026-01-01T00:60:00Z",
    "2026-01-01T00:00:60Z",
    "2026-01-01T00:00:00.+5Z",
  ] {
    assert_eq!(parse_iso(out_of_range), None, "{out_of_range}");
  }
}

#[test]
fn iso_millis_and_epoch_millis_print_like_date() {
  use super::{epoch_millis, iso_millis};
  let t = UNIX_EPOCH + Duration::from_millis(1_791_351_198_161);
  assert_eq!(iso_millis(t), "2026-10-07T05:33:18.161Z");
  assert_eq!(epoch_millis(t), 1_791_351_198_161);
  assert_eq!(
    iso_millis(UNIX_EPOCH + Duration::from_secs(1)),
    "1970-01-01T00:00:01.000Z"
  );
  assert_eq!(epoch_millis(UNIX_EPOCH - Duration::from_secs(1)), 0);
}
