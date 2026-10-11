use serde_json::{Value, json};

use super::{PERIOD, advance, apply, iso};
use crate::reduce::Reduced;

#[test]
fn the_first_check_anchors_the_schedule_and_later_checks_keep_the_slots() {
  let first = advance(&Value::Null, 1_000);
  assert_eq!(first, json!({"anchor": 1_000, "nextCheckAt": 1_180}));
  let slow = advance(&first, 1_000 + PERIOD + 70);
  assert_eq!(
    slow["nextCheckAt"],
    1_000 + 2 * PERIOD,
    "a late check keeps the grid"
  );
  let on_slot = advance(&first, 1_180);
  assert_eq!(
    on_slot["nextCheckAt"], 1_360,
    "a check on a slot moves to the next one"
  );
  let future = advance(&json!({"anchor": 5_000}), 1_000);
  assert_eq!(
    future["anchor"], 1_000,
    "an anchor after now restarts the grid"
  );
}

#[test]
fn six_hours_of_checks_land_exactly_one_period_apart() {
  let mut schedule = Value::Null;
  let mut now = 10_000;
  let mut seen = Vec::new();
  while now < 10_000 + 6 * 3_600 {
    schedule = advance(&schedule, now);
    let next = schedule["nextCheckAt"].as_u64().unwrap();
    seen.push(next);
    now = next + 7;
  }
  assert!(seen.windows(2).all(|pair| pair[1] - pair[0] == PERIOD));
  assert_eq!(seen.len(), 120);
}

#[test]
fn apply_moves_only_scheduled_ticks_and_always_reports_the_next_check() {
  let mut reduced = Reduced {
    state: json!({"schedule": null}),
    result: json!({"nextCheckAt": null}),
  };
  apply(&mut reduced, 1_760_000_000, false);
  assert_eq!(reduced.result["nextCheckAt"], Value::Null);
  apply(&mut reduced, 1_760_000_000, true);
  assert_eq!(reduced.state["schedule"]["anchor"], 1_760_000_000);
  assert_eq!(reduced.result["nextCheckAt"], iso(1_760_000_180));
  assert_eq!(iso(1_760_000_180), "2025-10-09T08:56:20Z");
  apply(&mut reduced, 1_760_000_100, false);
  assert_eq!(
    reduced.result["nextCheckAt"],
    iso(1_760_000_180),
    "an ad-hoc tick keeps it"
  );
}
