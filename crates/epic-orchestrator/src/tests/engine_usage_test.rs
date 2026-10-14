use std::time::{Duration, SystemTime};

use serde_json::Value;

use super::{on_tick, sample};
use crate::journal;
use crate::server::{Engine, Fault};

fn samples(engine: &Engine) -> Vec<Value> {
  journal::tail(&engine.paths.journal_dir(), engine.now)
    .unwrap()
    .iter()
    .filter(|row| row.name == "engine-usage")
    .map(|row| serde_json::from_str(&row.note).unwrap())
    .collect()
}

#[test]
fn a_sample_carries_peak_memory_and_cpu_and_the_tick_waits_fifteen_minutes() {
  let dir = tempfile::tempdir().unwrap();
  let mut engine = Engine::open(crate::paths::Paths::at(dir.path()), None, Fault::None).unwrap();
  sample(&mut engine, "start").unwrap();
  on_tick(&mut engine).unwrap();
  assert_eq!(
    samples(&engine).len(),
    1,
    "a tick right after start adds nothing"
  );
  engine.now = SystemTime::now() + Duration::from_mins(15);
  engine.world.now_ms = toolu_state::time::epoch_millis(engine.now);
  on_tick(&mut engine).unwrap();
  let rows = samples(&engine);
  assert_eq!(rows.len(), 2);
  assert_eq!(rows[0]["event"], "start");
  assert_eq!(rows[1]["event"], "sample");
  assert!(rows[1]["max_rss_bytes"].as_u64().unwrap() > 1_000_000);
  assert!(rows[1]["cpu_ms"].as_u64().is_some());
}
