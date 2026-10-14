use super::{human, parse_since};

#[test]
fn since_takes_a_date_or_a_utc_time() {
  assert_eq!(parse_since("1970-01-02").unwrap(), 86_400_000);
  assert_eq!(parse_since("1970-01-01T00:00:01Z").unwrap(), 1_000);
  assert!(parse_since("yesterday").unwrap_err().contains("YYYY-MM-DD"));
}

#[test]
fn durations_read_as_seconds_minutes_or_hours() {
  assert_eq!(human(59_999), "59 s");
  assert_eq!(human(600_000), "10 min");
  assert_eq!(human(5_460_000), "1 h 31 min");
}
