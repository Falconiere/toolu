use super::reset_from_headers;

const NOW: u64 = 1_700_000_000_000;

fn reset(pairs: &[(&str, &str)]) -> Option<u64> {
  reset_from_headers(
    |name| {
      pairs
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
    },
    NOW,
  )
}

#[test]
fn retry_after_seconds_count_from_now() {
  assert_eq!(reset(&[("retry-after", "30")]), Some(NOW + 30_000));
  assert_eq!(reset(&[("retry-after", "1.5")]), Some(NOW + 1500));
}

#[test]
fn retry_after_http_date_is_always_gmt() {
  assert_eq!(
    reset(&[("retry-after", "Wed, 21 Oct 2015 07:28:00 GMT")]),
    Some(1_445_412_480_000)
  );
  assert_eq!(
    reset(&[("retry-after", "Wed, 21 Oct 2015 07:28:00 PST")]),
    None
  );
  assert_eq!(reset(&[("retry-after", "soon")]), None);
}

#[test]
fn linear_epoch_millisecond_reset_applies_only_when_exhausted() {
  let ms = "1700000123456";
  assert_eq!(
    reset(&[
      ("x-ratelimit-requests-reset", ms),
      ("x-ratelimit-requests-remaining", "0")
    ]),
    Some(1_700_000_123_456)
  );
  assert_eq!(
    reset(&[
      ("x-ratelimit-requests-reset", ms),
      ("x-ratelimit-requests-remaining", "4")
    ]),
    None
  );
}

#[test]
fn github_epoch_second_reset_is_scaled_to_milliseconds() {
  assert_eq!(
    reset(&[
      ("x-ratelimit-reset", "1700000100"),
      ("x-ratelimit-remaining", "0")
    ]),
    Some(1_700_000_100_000)
  );
  assert_eq!(reset(&[("x-ratelimit-reset", "1700000100")]), None);
}
