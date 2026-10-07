//! Sustained-pressure decisions (`resources.test.ts`, `pressure-policy.test.ts`).

use super::{
  Pressure, ResourceSample, Ticks, advance_pressure, is_pressure, pressure_of, sample_of,
};
use crate::ledger::jq::parse_json;

fn incident(at: f64) -> ResourceSample {
  ResourceSample {
    at,
    cpus: 8.0,
    load: 40.43,
    available_bytes: 8e9,
    total_bytes: 32e9,
    steal: Some(0.8595),
    ticks: None,
  }
}

#[test]
fn incident_steal_holds_new_work_and_recovery_needs_sustained_headroom() {
  let mut pressure = advance_pressure(None, &incident(0.0)).unwrap();
  assert!(!pressure.held);
  pressure = advance_pressure(Some(&pressure), &incident(60_000.0)).unwrap();
  assert!(pressure.held);
  assert_eq!(
    pressure.reason.as_deref(),
    Some("sustained CPU/load or memory pressure")
  );
  let recovered = |at| ResourceSample {
    load: 1.0,
    steal: Some(0.0),
    ..incident(at)
  };
  pressure = advance_pressure(Some(&pressure), &recovered(90_000.0)).unwrap();
  assert!(pressure.held);
  pressure = advance_pressure(Some(&pressure), &recovered(210_000.0)).unwrap();
  assert!(!pressure.held && pressure.reason.is_none());
  let nan = advance_pressure(Some(&pressure), &incident(f64::NAN));
  assert_eq!(nan, Err("invalid resource pressure sample".to_owned()));
  let backwards = advance_pressure(Some(&pressure), &incident(1.0));
  assert_eq!(
    backwards,
    Err("resource pressure sample moved backwards".to_owned())
  );
}

#[test]
fn low_memory_holds_after_a_minute() {
  let low = |at| ResourceSample {
    at,
    cpus: 10.0,
    load: 1.0,
    available_bytes: 1e9,
    total_bytes: 32e9,
    steal: None,
    ticks: None,
  };
  let first = advance_pressure(None, &low(0.0)).unwrap();
  assert_eq!(
    (first.held, first.bad_since, first.good_since),
    (false, Some(0.0), None)
  );
  assert!(advance_pressure(Some(&first), &low(60_000.0)).unwrap().held);
}

/// A pressure record TypeScript wrote on this machine.
const RECORDED: &str = r#"{"held":false,"badSince":null,"goodSince":1791351198161,"sample":{"at":1791351198161,"cpus":8,"load":5.88,"availableBytes":14005608448,"totalBytes":33648418816,"steal":0.029921259842519685,"ticks":{"total":311292808,"steal":71603453}},"reason":null}"#;

#[test]
fn a_recorded_pressure_round_trips_byte_for_byte() {
  let value = parse_json(RECORDED).unwrap();
  let pressure: Pressure = pressure_of(&value).unwrap();
  assert_eq!(
    pressure.sample.ticks,
    Some(Ticks {
      total: 311_292_808.0,
      steal: 71_603_453.0
    })
  );
  assert_eq!(pressure.to_ordered().to_text(false), RECORDED);
}

#[test]
fn malformed_pressure_and_samples_are_rejected() {
  for body in [
    RECORDED.replace("5.88", "\"high\""),
    RECORDED.replace(r#""reason":null"#, r#""reason":"x""#),
    RECORDED.replace(r#""held":false"#, r#""held":true"#),
    RECORDED.replace(
      r#""goodSince":1791351198161"#,
      r#""goodSince":1791351198162"#,
    ),
    RECORDED.replace(r#""cpus":8"#, r#""cpus":0"#),
    RECORDED.replace(r#""steal":0.029921259842519685"#, r#""steal":1.5"#),
    RECORDED.replace(
      r#""availableBytes":14005608448"#,
      r#""availableBytes":99999999999"#,
    ),
    RECORDED.replace(r#""total":311292808"#, r#""total":-1"#),
    RECORDED.replace(r#","badSince":null"#, ""),
  ] {
    assert!(!is_pressure(&parse_json(&body).unwrap()), "{body}");
  }
  assert_eq!(sample_of(&parse_json("[]").unwrap()), None);
  assert!(!is_pressure(&parse_json("3").unwrap()));
}
