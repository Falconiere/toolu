//! The GitHub half of the scripted-epic budgets (#451): a pull request that
//! waits through keep-going checks and then turns green, measured from the
//! engine's own journal.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use toolu_http_test_support::{Fixture, Reply};
use toolu_state::time::epoch_millis;

use super::fixture::{engine, graphql_body, rest_routes};
use crate::journal::{self, Record};
use crate::metrics::{self, Query};
use crate::server::Engine;
use crate::watch::{self, CHECK_MS};

fn at(engine: &mut Engine, ms: u64) {
  engine.world.now_ms = ms;
  engine.now = UNIX_EPOCH + Duration::from_millis(ms);
}

fn budget(doc: &Value, id: &str) -> (String, Value) {
  let rows = metrics::budget_rows(doc);
  let row = rows.iter().find(|row| row["id"] == id).expect("budget row");
  (
    row["status"].as_str().unwrap_or("").to_owned(),
    row["value"].clone(),
  )
}

#[test]
fn a_waiting_pr_costs_no_prompt_and_its_green_verdict_is_acted_on_at_once() {
  let fixture = Fixture::start().expect("fixture");
  rest_routes(&fixture);
  let pending = Reply::new(200, graphql_body(false));
  let green = Reply::new(200, graphql_body(true));
  fixture
    .sequence("/graphql", vec![pending.clone(), pending, green])
    .expect("GraphQL route");
  let (_temp, mut engine, tick) = engine(&fixture);
  let base = epoch_millis(SystemTime::now());
  at(&mut engine, base);
  engine.world.watches.clear();
  watch::sync(&mut engine.world);
  for step in 0..3 {
    at(&mut engine, base + step * CHECK_MS);
    engine.github_tick(&tick);
    engine.pump().expect("pump");
  }
  let dir = engine.paths.journal_dir();
  let query = Query {
    epic: None,
    since_ms: None,
    now_ms: base + 3 * CHECK_MS,
  };
  let doc = metrics::document(&dir, &query).expect("metrics");
  let names: Vec<String> = journal::tail(&dir, engine.now)
    .expect("journal")
    .into_iter()
    .map(|row| row.name)
    .collect();
  assert_eq!(doc["prompts"]["decided_checks"], 3, "{names:?}");
  assert_eq!(budget(&doc, "prompts-while-waiting").0, "pass");
  assert_eq!(
    budget(&doc, "check-gap"),
    ("pass".to_owned(), CHECK_MS.into())
  );
  assert_eq!(doc["github"]["reaction_ms"]["count"], 1, "{names:?}");
  assert_eq!(budget(&doc, "verdict-reaction").0, "pass");
  // A regression that prompts the worker on a keep-going check fails the budget.
  let planted = Record::new("prompt", "babysit", "one", "", "keep going");
  journal::append(&dir, &engine.paths.journal_lock(), &planted, engine.now).expect("plant");
  let doc = metrics::document(&dir, &query).expect("metrics");
  assert_eq!(
    budget(&doc, "prompts-while-waiting"),
    ("fail".to_owned(), 1.into())
  );
}
