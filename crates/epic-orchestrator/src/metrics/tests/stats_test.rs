use serde_json::json;

use super::{decimal, pct, share, total};

#[test]
fn percentiles_interpolate_between_ranks_and_round_down() {
  assert_eq!(
    pct(&[4, 1, 3, 2]),
    json!({"count": 4, "p50": 2, "p90": 3, "max": 4})
  );
  assert_eq!(
    pct(&[100, 200]),
    json!({"count": 2, "p50": 150, "p90": 190, "max": 200})
  );
}

#[test]
fn one_value_is_every_percentile_and_none_is_null() {
  assert_eq!(pct(&[7]), json!({"count": 1, "p50": 7, "p90": 7, "max": 7}));
  assert_eq!(
    pct(&[]),
    json!({"count": 0, "p50": null, "p90": null, "max": null})
  );
}

#[test]
fn shares_keep_four_decimals_and_a_zero_total_is_null() {
  assert_eq!(share(1, 3), json!(0.3333));
  assert_eq!(share(5, 5), json!(1.0));
  assert_eq!(share(1, 0), json!(null));
  assert_eq!(decimal(5422, 10), json!(542.2));
}

#[test]
fn totals_saturate_instead_of_overflowing() {
  assert_eq!(total(&[1, 2, 3]), 6);
  assert_eq!(total(&[u64::MAX, 5]), u64::MAX);
  assert_eq!(total(&[]), 0);
}
