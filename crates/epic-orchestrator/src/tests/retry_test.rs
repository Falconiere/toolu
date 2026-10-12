use std::cell::Cell;
use std::time::Instant;

use toolu_runtime::env::Env;

use super::{Budget, Class, Failure, Policy, map_limit, now_ms, number, with_retry};

const FAST: Policy = Policy {
  attempts: 4,
  base_ms: 1,
  max_sleep_ms: 2000,
};

/// A failure of `class` with `message`.
fn failure(message: &str, class: Class) -> Failure {
  Failure {
    class,
    message: message.to_owned(),
    reset_at_ms: None,
  }
}

fn transient() -> Failure {
  failure("HTTP 503", Class::Transient)
}

#[test]
fn with_retry_retries_transient_failures_then_succeeds() {
  let calls = Cell::new(0);
  let out = with_retry(&FAST, &mut Budget::unbounded(), false, || {
    calls.set(calls.get() + 1);
    if calls.get() < 3 {
      Err(transient())
    } else {
      Ok("ok")
    }
  });
  assert_eq!(out, Ok("ok"));
  assert_eq!(calls.get(), 3);
}

#[test]
fn with_retry_does_not_retry_a_permanent_failure() {
  let calls = Cell::new(0);
  let out: Result<(), String> = with_retry(&FAST, &mut Budget::unbounded(), false, || {
    calls.set(calls.get() + 1);
    Err(failure("HTTP 404", Class::Permanent))
  });
  assert_eq!(out, Err("HTTP 404".to_owned()));
  assert_eq!(calls.get(), 1);
}

#[test]
fn with_retry_never_repeats_a_write_whose_5xx_may_have_applied() {
  let calls = Cell::new(0);
  let out: Result<(), String> = with_retry(&FAST, &mut Budget::unbounded(), true, || {
    calls.set(calls.get() + 1);
    Err(failure("HTTP 502", Class::Transient))
  });
  assert_eq!(out, Err("HTTP 502".to_owned()));
  assert_eq!(calls.get(), 1);
}

#[test]
fn with_retry_waits_for_the_reset_a_rate_limit_gives() {
  let calls = Cell::new(0);
  let started = Instant::now();
  let reset = now_ms() + 50;
  let out = with_retry(&FAST, &mut Budget::unbounded(), true, || {
    calls.set(calls.get() + 1);
    if calls.get() == 1 {
      Err(Failure {
        message: "HTTP 429".into(),
        class: Class::RateLimit,
        reset_at_ms: Some(reset),
      })
    } else {
      Ok(())
    }
  });
  assert_eq!(out, Ok(()));
  assert!(
    started.elapsed().as_millis() >= 1000,
    "waits reset plus a second"
  );
}

#[test]
fn with_retry_refuses_a_reset_beyond_the_cap_without_sleeping() {
  let started = Instant::now();
  let out: Result<(), String> = with_retry(&FAST, &mut Budget::unbounded(), false, || {
    Err(Failure {
      message: "HTTP 429".into(),
      class: Class::RateLimit,
      reset_at_ms: Some(now_ms() + 3_600_000),
    })
  });
  let err = out.expect_err("cap");
  assert!(err.starts_with("rate limited until "), "{err}");
  assert!(err.ends_with("; longer than the 2s cap"), "{err}");
  assert!(started.elapsed().as_millis() < 500);
}

#[test]
fn an_effect_budget_stops_sleeping_once_spent() {
  let policy = Policy {
    attempts: 5,
    base_ms: 1,
    max_sleep_ms: 3000,
  };
  let calls = Cell::new(0);
  let started = Instant::now();
  let out: Result<(), String> = with_retry(&policy, &mut policy.effect(), false, || {
    calls.set(calls.get() + 1);
    Err(Failure {
      message: "HTTP 429".into(),
      class: Class::RateLimit,
      reset_at_ms: Some(now_ms() + 1000),
    })
  });
  assert_eq!(out, Err("HTTP 429; the sleep budget is spent".to_owned()));
  assert_eq!(calls.get(), 2);
  let elapsed = started.elapsed().as_millis();
  assert!((1900..2900).contains(&elapsed), "{elapsed}");
}

#[test]
fn a_rate_limit_without_a_reset_waits_at_least_a_minute() {
  let mut budget = Budget { remaining_ms: 10 };
  let out: Result<(), String> = with_retry(&FAST, &mut budget, false, || {
    Err(failure("secondary rate limit", Class::RateLimit))
  });
  assert_eq!(
    out,
    Err("secondary rate limit; the sleep budget is spent".to_owned())
  );
}

#[test]
fn policy_reads_the_environment_and_falls_back_on_bad_values() {
  let env = Env::from_pairs([
    ("EPIC_API_ATTEMPTS", "2"),
    ("EPIC_API_BACKOFF_MS", "-5"),
    ("EPIC_API_MAX_SLEEP_S", "1.5"),
  ]);
  assert_eq!(
    Policy::from_env(&env),
    Policy {
      attempts: 2,
      base_ms: 2000,
      max_sleep_ms: 1000
    }
  );
  assert_eq!(
    Policy::from_env(&Env::default()),
    Policy {
      attempts: 5,
      base_ms: 2000,
      max_sleep_ms: 900_000
    }
  );
  assert_eq!(number(&Env::from_pairs([("N", "NaN")]), "N", 7), 7);
  assert_eq!(number(&Env::from_pairs([("N", "1e30")]), "N", 7), u64::MAX);
}

#[test]
fn map_limit_keeps_order_caps_concurrency_and_returns_the_first_failure() {
  let items: Vec<u32> = (1..=9).collect();
  let peak = std::sync::atomic::AtomicUsize::new(0);
  let live = std::sync::atomic::AtomicUsize::new(0);
  let out = map_limit(&items, 4, |item| {
    let now = live.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    peak.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(5));
    live.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    Ok(item * 2)
  });
  assert_eq!(
    out,
    Ok(items.iter().map(|item| item * 2).collect::<Vec<_>>())
  );
  assert!(peak.load(std::sync::atomic::Ordering::SeqCst) <= 4);
  let failed = map_limit(&items, 4, |item| {
    if *item == 3 || *item == 7 {
      Err(format!("bad {item}"))
    } else {
      Ok(*item)
    }
  });
  assert_eq!(failed, Err("bad 3".to_owned()));
}
