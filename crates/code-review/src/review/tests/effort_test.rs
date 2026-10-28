use std::cell::Cell;

use super::{EffortSource, choose, floor, from_score, metadata};
use crate::budget::Effort;
use crate::contract::{RedactionSpan, Side, Source};
use crate::prepass::Redaction;
use crate::review::tests::{prepass, shaped, trusted};
use crate::shape::Rename;

#[test]
fn every_floor_names_its_reason_and_plain_code_has_none() {
  let clean = prepass(Vec::new());
  let floor_of = |path: &str| floor(&shaped(&[(path, &["x"])]), &clean);
  assert_eq!(floor_of("src/lib.rs"), None);
  assert_eq!(floor_of("docs/guide.md"), None);
  assert_eq!(
    floor_of(".github/workflows/tests.yml").as_deref(),
    Some("CI workflow change")
  );
  for gate in [
    "tooling/conventions/guardrails/rust/layers.json",
    "clippy.toml",
    "crates/x/deny.toml",
    "lefthook.yml",
  ] {
    assert_eq!(
      floor_of(gate).as_deref(),
      Some("gate policy change"),
      "{gate}"
    );
  }
  for secure in [
    "src/Auth.rs",
    "core/secrets.rs",
    "lib/token_store.rs",
    "crypto/aes.rs",
    "src/password.rs",
  ] {
    assert_eq!(
      floor_of(secure).as_deref(),
      Some("credential or security code"),
      "{secure}"
    );
  }
  let mut renamed = shaped(&[("src/plain.rs", &["x"])]);
  renamed.renames.push(Rename {
    old_path: ".github/workflows/old.yml".into(),
    path: "src/plain.rs".into(),
  });
  assert_eq!(
    floor(&renamed, &clean).as_deref(),
    Some("CI workflow change")
  );
  let lines: Vec<String> = (0..=1000).map(|index| format!("line {index}")).collect();
  let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
  assert_eq!(
    floor(&shaped(&[("src/big.rs", &refs)]), &clean).as_deref(),
    Some("diff above 1000 changed lines")
  );
  assert_eq!(floor(&shaped(&[("src/big.rs", &refs[1..])]), &clean), None);
}

#[test]
fn a_secret_hit_or_a_redaction_floors_the_run() {
  let diff = shaped(&[("src/lib.rs", &["x"])]);
  assert_eq!(
    floor(&diff, &prepass(vec![trusted(Source::Secrets)])).as_deref(),
    Some("secret detected by the pre-pass")
  );
  assert_eq!(floor(&diff, &prepass(vec![trusted(Source::AstGrep)])), None);
  let mut removed = prepass(Vec::new());
  removed.redactions.push(Redaction {
    path: "src/lib.rs".into(),
    side: Side::Old,
    line: 1,
    spans: vec![RedactionSpan { start: 0, end: 1 }],
  });
  assert!(floor(&diff, &removed).is_some());
}

#[test]
fn the_score_is_nudged_up_before_rounding() {
  let cases = [
    (0.0, Effort::Small),
    (0.34, Effort::Small),
    (0.35, Effort::Medium),
    (1.4, Effort::High),
    (2.2, Effort::High),
    (2.4, Effort::Xhigh),
    (3.0, Effort::Xhigh),
  ];
  for (score, level) in cases {
    assert_eq!(from_score(score), level, "{score}");
  }
}

#[test]
fn an_explicit_level_wins_and_never_asks_jev() {
  let asked = Cell::new(false);
  let ask = || {
    asked.set(true);
    Ok(3.0)
  };
  let choice = choose(
    Some((Effort::Small, EffortSource::Flag)),
    Some("CI workflow change".into()),
    ask,
  );
  assert!(!asked.get());
  assert_eq!(
    (choice.level, choice.source),
    (Effort::Small, EffortSource::Flag)
  );
  assert_eq!(
    choice.reason,
    "explicit level; floor high would apply: CI workflow change"
  );
  let config = choose(Some((Effort::Xhigh, EffortSource::Config)), None, || {
    Ok(0.0)
  });
  assert_eq!(
    (config.level, config.source, config.floor),
    (Effort::Xhigh, EffortSource::Config, None)
  );
  let auto = choose(Some((Effort::Auto, EffortSource::Config)), None, || Ok(0.2));
  assert_eq!(
    (auto.level, auto.source),
    (Effort::Small, EffortSource::Auto)
  );
}

#[test]
fn auto_is_floored_and_falls_back_to_medium() {
  let floored = choose(None, Some("CI workflow change".into()), || Ok(0.2));
  assert_eq!(
    (floored.level, floored.source),
    (Effort::High, EffortSource::Auto)
  );
  let raised = choose(None, Some("gate policy change".into()), || Ok(3.0));
  assert_eq!(raised.level, Effort::Xhigh);
  let fallback = choose(None, None, || Err("HTTP 500".into()));
  assert_eq!(
    (fallback.level, fallback.source),
    (Effort::Medium, EffortSource::Fallback)
  );
  assert_eq!(fallback.reason, "medium fallback: HTTP 500");
  let both = choose(None, Some("secret detected by the pre-pass".into()), || {
    Err("deadline".into())
  });
  assert_eq!(both.level, Effort::High);
  let doc = serde_json::to_value(&both).unwrap();
  assert_eq!(doc["source"], "fallback");
  assert_eq!(doc["floor"], "secret detected by the pre-pass");
  assert!(
    serde_json::to_value(&fallback)
      .unwrap()
      .get("floor")
      .is_none()
  );
}

#[test]
fn the_selector_state_is_metadata_without_source() {
  let diff = shaped(&[("src/a.rs", &["let secret_value = 42;", "two"])]);
  let state = metadata(&diff, &prepass(vec![trusted(Source::AstGrep)]));
  assert_eq!(state["files"][0]["added"], 2);
  assert_eq!(state["files"][0]["removed"], 0);
  assert_eq!(state["prepass"]["ast_grep"], 1);
  assert!(!state.to_string().contains("secret_value"));
}
